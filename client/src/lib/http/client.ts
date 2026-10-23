/**
 * Cliente HTTP sobre la API estándar de Fetch. Sin dependencias.
 *
 * El transporte es inyectable (`fetch`): por defecto usa el `fetch` global, pero para hablar con
 * el agente se le pasa uno que se ejecuta en Rust (p. ej. el de `@tauri-apps/plugin-http`, con la
 * misma firma). La webview nunca llega al agente directamente: la CSP solo permite IPC y el agente
 * rechaza cualquier petición con `Origin` (protección contra CSRF / DNS rebinding del túnel).
 *
 * Este módulo no importa nada en tiempo de ejecución, para poder probarlo con `node --test`.
 */

/** Espejo de `shared::ErrorCode` (shared/src/error.rs). Debe mantenerse sincronizado. */
export const API_ERROR_CODES = [
    'BAD_REQUEST',
    'INVALID_JSON',
    'UNSUPPORTED_MEDIA_TYPE',
    'PAYLOAD_TOO_LARGE',
    'UNAUTHORIZED',
    'FORBIDDEN',
    'NOT_FOUND',
    'METHOD_NOT_ALLOWED',
    'CONFLICT',
    'REQUEST_TIMEOUT',
    'OVERLOADED',
    'UNAVAILABLE',
    'INTERNAL',
    'SERVICE_NOT_FOUND',
    'UNIT_NOT_FOUND',
    'UNIT_MASKED',
    'UNIT_REFUSES_MANUAL_OPERATION',
    'SERVICE_JOB_FAILED',
    'SYSTEMD_PERMISSION_DENIED',
    'SYSTEMD_TIMEOUT',
    'SYSTEMD_FAILED',
    'LOG_STREAM_LIMIT',
    'WEB_SOCKET_REQUIRED',
    'SERVICE_ALREADY_EXISTS',
    'INVALID_ENVIRONMENT',
    'ENVIRONMENT_FILE_INVALID',
    'INVALID_DEPLOYMENT',
    'REPOSITORY_LOCKED',
    'DEPLOYMENT_INCOMPLETE',
    'DEPLOY_KEY_MISSING',
    'DEPLOY_IN_PROGRESS',
    'NOT_DEPLOYED',
    'NOTHING_TO_ROLL_BACK',
    'NOTHING_TO_RUN',
    'INVALID_SITE',
    'INVALID_LIMITS',
    'DOMAIN_IN_USE',
    'NGINX_REJECTED',
    'NGINX_UNAVAILABLE',
    'UNKNOWN',
] as const;

export type ApiErrorCode = (typeof API_ERROR_CODES)[number];

const KNOWN_CODES = new Set<string>(API_ERROR_CODES);

// --- Errores ---

/** Base de todos los errores del cliente: permite un único `instanceof` en los `catch`. */
export class RequestError extends Error {
    readonly method: string;
    readonly url: string;
    readonly requestId: string;

    constructor(message: string, method: string, url: string, requestId: string, options?: ErrorOptions) {
        super(message, options);
        this.name = new.target.name;
        this.method = method;
        this.url = url;
        this.requestId = requestId;
    }
}

/** El servidor respondió con un estado fuera de 2xx. */
export class HttpError extends RequestError {
    readonly status: number;
    /** Código estable del agente. `UNKNOWN` si la respuesta no traía uno reconocible. */
    readonly code: ApiErrorCode;
    /** Mensaje técnico del servidor: para el registro, nunca para mostrar al usuario. */
    readonly detail: string | undefined;

    constructor(
        status: number,
        code: ApiErrorCode,
        detail: string | undefined,
        method: string,
        url: string,
        requestId: string,
    ) {
        super(`${method} ${url} respondió ${status} (${code})`, method, url, requestId);
        this.status = status;
        this.code = code;
        this.detail = detail;
    }
}

/** No hubo respuesta: conexión rechazada, túnel caído, DNS… */
export class NetworkError extends RequestError {}

/** Se superó `timeoutMs`. */
export class TimeoutError extends RequestError {}

/** La cancelación vino del llamador (su `signal`). No suele mostrarse al usuario. */
export class AbortedError extends RequestError {}

/** La respuesta 2xx no tenía la forma esperada (JSON inválido o `parse` falló). */
export class ResponseFormatError extends RequestError {}

// --- Tipos públicos ---

export type QueryValue = string | number | boolean | null | undefined;
export type Query = Readonly<Record<string, QueryValue | readonly QueryValue[]>>;

export interface RetryOptions {
    /** Intentos adicionales tras el primero. 0 desactiva los reintentos. */
    readonly retries: number;
    /** Espera base del backoff exponencial (con jitter). */
    readonly baseDelayMs: number;
    /** Tope de espera entre intentos, también para `Retry-After`. */
    readonly maxDelayMs: number;
}

export interface HttpClientOptions {
    /** URL absoluta base, p. ej. `http://127.0.0.1:4040/api/v1/`. */
    readonly baseUrl: string;
    /** Transporte. Por defecto, `globalThis.fetch`. */
    readonly fetch?: typeof fetch;
    /** Por defecto 15 s. */
    readonly timeoutMs?: number;
    readonly headers?: HeadersInit;
    readonly retry?: Partial<RetryOptions>;
    /** Para pruebas: sustituye la espera entre reintentos. */
    readonly sleep?: (ms: number, signal: AbortSignal) => Promise<void>;
}

export interface RequestOptions<T> {
    readonly query?: Query;
    /** Objetos y arrays se envían como JSON; `FormData`, `Blob`, `URLSearchParams` y `string` tal cual. */
    readonly body?: unknown;
    readonly headers?: HeadersInit;
    readonly signal?: AbortSignal;
    readonly timeoutMs?: number;
    /** Sobrescribe la política de reintentos para esta petición. */
    readonly retry?: Partial<RetryOptions>;
    /**
     * Valida y tipa la respuesta. Sin `parse`, el tipo `T` es solo una afirmación: los datos que
     * llegan de fuera son una frontera de confianza y conviene validarlos.
     */
    readonly parse?: (data: unknown) => T;
}

export type Method = 'GET' | 'HEAD' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

export interface HttpClient {
    request<T = unknown>(method: Method, path: string, options?: RequestOptions<T>): Promise<T>;
    get<T = unknown>(path: string, options?: RequestOptions<T>): Promise<T>;
    post<T = unknown>(path: string, options?: RequestOptions<T>): Promise<T>;
    put<T = unknown>(path: string, options?: RequestOptions<T>): Promise<T>;
    patch<T = unknown>(path: string, options?: RequestOptions<T>): Promise<T>;
    delete<T = unknown>(path: string, options?: RequestOptions<T>): Promise<T>;
}

// --- Implementación ---

const DEFAULT_TIMEOUT_MS = 15_000;
const DEFAULT_RETRY: RetryOptions = { retries: 2, baseDelayMs: 250, maxDelayMs: 5_000 };
/** Solo se reintentan métodos idempotentes: repetir un POST podría duplicar una acción. */
const RETRYABLE_METHODS = new Set<Method>(['GET', 'HEAD', 'PUT', 'DELETE']);
const RETRYABLE_STATUS = new Set([408, 429, 502, 503, 504]);

export function createHttpClient(options: HttpClientOptions): HttpClient {
    const base = new URL(options.baseUrl);
    if (!base.pathname.endsWith('/')) base.pathname += '/';
    const transport = options.fetch ?? globalThis.fetch.bind(globalThis);
    const sleep = options.sleep ?? abortableSleep;
    const defaultTimeout = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    const defaultRetry = { ...DEFAULT_RETRY, ...options.retry };

    async function request<T>(method: Method, path: string, init: RequestOptions<T> = {}): Promise<T> {
        const url = resolveUrl(base, path, init.query);
        const requestId = newRequestId();
        const retry = RETRYABLE_METHODS.has(method)
            ? { ...defaultRetry, ...init.retry }
            : { ...defaultRetry, retries: 0 };
        const headers = new Headers(options.headers);
        new Headers(init.headers).forEach((value, key) => headers.set(key, value));
        headers.set('accept', 'application/json');
        headers.set('x-request-id', requestId);
        const body = encodeBody(init.body, headers);

        for (let attempt = 0; ; attempt++) {
            const outcome = await attemptOnce(method, url, requestId, headers, body, init, defaultTimeout);
            const canRetry = attempt < retry.retries && !init.signal?.aborted;
            if (outcome.kind === 'response') {
                const { response } = outcome;
                if (response.ok) return readBody(response, method, url, requestId, init.parse);
                if (canRetry && RETRYABLE_STATUS.has(response.status)) {
                    await discard(response);
                    await wait(
                        sleep,
                        backoff(attempt, retry, response.headers.get('retry-after')),
                        init.signal,
                        method,
                        url,
                        requestId,
                    );
                    continue;
                }
                throw await toHttpError(response, method, url, requestId);
            }
            if (canRetry && (outcome.error instanceof NetworkError || outcome.error instanceof TimeoutError)) {
                await wait(sleep, backoff(attempt, retry, null), init.signal, method, url, requestId);
                continue;
            }
            throw outcome.error;
        }
    }

    async function attemptOnce<T>(
        method: Method,
        url: URL,
        requestId: string,
        headers: Headers,
        body: BodyInit | undefined,
        init: RequestOptions<T>,
        defaultTimeoutMs: number,
    ): Promise<{ kind: 'response'; response: Response } | { kind: 'error'; error: RequestError }> {
        const controller = new AbortController();
        let timedOut = false;
        const timer = setTimeout(() => {
            timedOut = true;
            controller.abort();
        }, init.timeoutMs ?? defaultTimeoutMs);
        const onAbort = () => controller.abort();
        init.signal?.addEventListener('abort', onAbort, { once: true });
        if (init.signal?.aborted) controller.abort();

        try {
            const response = await transport(url, {
                method,
                headers,
                body,
                signal: controller.signal,
                redirect: 'error',
            });
            return { kind: 'response', response };
        } catch (cause) {
            const where = [method, url.href, requestId] as const;
            if (timedOut)
                return { kind: 'error', error: new TimeoutError('tiempo de espera agotado', ...where, { cause }) };
            if (init.signal?.aborted)
                return { kind: 'error', error: new AbortedError('petición cancelada', ...where, { cause }) };
            return { kind: 'error', error: new NetworkError('sin respuesta del servidor', ...where, { cause }) };
        } finally {
            clearTimeout(timer);
            init.signal?.removeEventListener('abort', onAbort);
        }
    }

    return {
        request,
        get: (path, init) => request('GET', path, init),
        post: (path, init) => request('POST', path, init),
        put: (path, init) => request('PUT', path, init),
        patch: (path, init) => request('PATCH', path, init),
        delete: (path, init) => request('DELETE', path, init),
    };
}

/**
 * `path` siempre es relativo a `baseUrl`. Una ruta absoluta (`https://…`, `//host`) o que intente
 * salir de la base (`../`) se rechaza: el cliente nunca debe acabar hablando con otro origen.
 */
export function resolveUrl(base: URL, path: string, query?: Query): URL {
    if (/^[a-z][a-z\d+.-]*:|^\/\//i.test(path)) {
        throw new TypeError(`la ruta debe ser relativa a la base: ${path}`);
    }
    const url = new URL(path.replace(/^\/+/, ''), base);
    if (url.origin !== base.origin || !url.pathname.startsWith(base.pathname)) {
        throw new TypeError(`la ruta sale de la base: ${path}`);
    }
    for (const [key, raw] of Object.entries(query ?? {})) {
        for (const value of Array.isArray(raw) ? raw : [raw]) {
            if (value !== undefined && value !== null) url.searchParams.append(key, String(value));
        }
    }
    return url;
}

function encodeBody(body: unknown, headers: Headers): BodyInit | undefined {
    if (body === undefined) return undefined;
    if (
        typeof body === 'string' ||
        body instanceof FormData ||
        body instanceof Blob ||
        body instanceof URLSearchParams ||
        body instanceof ArrayBuffer
    ) {
        return body;
    }
    headers.set('content-type', 'application/json');
    return JSON.stringify(body);
}

async function readBody<T>(
    response: Response,
    method: string,
    url: URL,
    requestId: string,
    parse: ((data: unknown) => T) | undefined,
): Promise<T> {
    const text = response.status === 204 || method === 'HEAD' ? '' : await response.text();
    let data: unknown = undefined;
    if (text !== '') {
        const isJson = (response.headers.get('content-type') ?? '').includes('json');
        try {
            data = isJson ? JSON.parse(text) : text;
        } catch (cause) {
            throw new ResponseFormatError('la respuesta no es JSON válido', method, url.href, requestId, { cause });
        }
    }
    if (parse === undefined) return data as T;
    try {
        return parse(data);
    } catch (cause) {
        throw new ResponseFormatError('la respuesta no tiene la forma esperada', method, url.href, requestId, {
            cause,
        });
    }
}

async function toHttpError(response: Response, method: string, url: URL, requestId: string): Promise<HttpError> {
    let code: ApiErrorCode = 'UNKNOWN';
    let detail: string | undefined;
    try {
        const body: unknown = await response.json();
        if (typeof body === 'object' && body !== null) {
            const { code: raw, message } = body as Record<string, unknown>;
            if (typeof raw === 'string' && KNOWN_CODES.has(raw)) code = raw as ApiErrorCode;
            if (typeof message === 'string') detail = message;
        }
    } catch {
        // Cuerpo vacío o no JSON (p. ej. un 502 de un proxy): se queda en UNKNOWN.
    }
    return new HttpError(response.status, code, detail, method, url.href, requestId);
}

async function discard(response: Response): Promise<void> {
    try {
        await response.body?.cancel();
    } catch {
        // Nada que liberar.
    }
}

function backoff(attempt: number, retry: RetryOptions, retryAfter: string | null): number {
    const seconds = retryAfter === null ? Number.NaN : Number(retryAfter);
    if (Number.isFinite(seconds) && seconds >= 0) return Math.min(seconds * 1000, retry.maxDelayMs);
    const exponential = retry.baseDelayMs * 2 ** attempt;
    // "Full jitter": evita que varios clientes reintenten a la vez.
    return Math.min(retry.maxDelayMs, Math.random() * exponential);
}

async function wait(
    sleep: NonNullable<HttpClientOptions['sleep']>,
    ms: number,
    signal: AbortSignal | undefined,
    method: string,
    url: URL,
    requestId: string,
): Promise<void> {
    const controller = new AbortController();
    const onAbort = () => controller.abort();
    signal?.addEventListener('abort', onAbort, { once: true });
    try {
        await sleep(ms, controller.signal);
    } finally {
        signal?.removeEventListener('abort', onAbort);
    }
    if (signal?.aborted) throw new AbortedError('petición cancelada', method, url.href, requestId);
}

function abortableSleep(ms: number, signal: AbortSignal): Promise<void> {
    return new Promise((resolve) => {
        const timer = setTimeout(resolve, ms);
        signal.addEventListener(
            'abort',
            () => {
                clearTimeout(timer);
                resolve();
            },
            { once: true },
        );
    });
}

/** Se envía como `x-request-id`: el agente lo registra, así se puede seguir una petición. */
function newRequestId(): string {
    return globalThis.crypto.randomUUID();
}
