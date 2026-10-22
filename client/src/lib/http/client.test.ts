import { describe, expect, test } from 'vitest';

import {
    AbortedError,
    createHttpClient,
    HttpError,
    NetworkError,
    ResponseFormatError,
    resolveUrl,
    TimeoutError,
} from './client';

type Call = { url: string; init: RequestInit };

/** Transporte falso: responde en orden con las respuestas (o errores) dados. */
function fakeFetch(...replies: (Response | Error)[]) {
    const calls: Call[] = [];
    const fetch = (async (url: URL, init: RequestInit) => {
        calls.push({ url: String(url), init });
        const reply = replies.shift();
        if (reply === undefined) throw new Error('sin más respuestas');
        if (reply instanceof Error) throw reply;
        return reply;
    }) as unknown as typeof globalThis.fetch;
    return { fetch, calls };
}

const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
    new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', ...headers } });

const noSleep = async () => {};
const client = (fetch: typeof globalThis.fetch, extra = {}) =>
    createHttpClient({ baseUrl: 'http://127.0.0.1:4040/api/v1', fetch, sleep: noSleep, ...extra });

const headerOf = (call: Call | undefined, name: string) => new Headers(call?.init.headers).get(name);

describe('resolveUrl', () => {
    const base = new URL('http://127.0.0.1:4040/api/v1/');

    test('resuelve rutas relativas y serializa la query', () => {
        const url = resolveUrl(base, '/services/nginx', { lines: 50, unit: ['a', 'b'], skip: undefined, none: null });
        expect(url.href).toBe('http://127.0.0.1:4040/api/v1/services/nginx?lines=50&unit=a&unit=b');
    });

    test.each(['https://evil.example/x', '//evil.example/x', 'javascript:alert(1)', '../../admin', '..'])(
        'nunca sale del origen ni de la base: %s',
        (path) => {
            expect(() => resolveUrl(base, path)).toThrow(TypeError);
        },
    );
});

describe('createHttpClient', () => {
    test('GET devuelve el JSON y envía cabeceras de control', async () => {
        const { fetch, calls } = fakeFetch(json({ status: 'ready' }));
        await expect(client(fetch).get('health')).resolves.toEqual({ status: 'ready' });
        expect(headerOf(calls[0], 'accept')).toBe('application/json');
        expect(headerOf(calls[0], 'x-request-id')).toMatch(/^[0-9a-f-]{36}$/);
        expect(calls[0]?.init.redirect).toBe('error');
    });

    test('POST serializa objetos como JSON', async () => {
        const { fetch, calls } = fakeFetch(new Response(null, { status: 204 }));
        await expect(client(fetch).post('deploy', { body: { branch: 'main' } })).resolves.toBeUndefined();
        expect(calls[0]?.init.body).toBe('{"branch":"main"}');
        expect(headerOf(calls[0], 'content-type')).toBe('application/json');
    });

    test('los errores del agente conservan su código estable', async () => {
        const { fetch } = fakeFetch(json({ code: 'NOT_FOUND', message: 'recurso no encontrado' }, 404));
        const error = await client(fetch)
            .get('services/x')
            .catch((caught: unknown) => caught);
        expect(error).toBeInstanceOf(HttpError);
        expect(error).toMatchObject({ status: 404, code: 'NOT_FOUND', detail: 'recurso no encontrado' });
    });

    test('un código desconocido o un cuerpo no JSON es UNKNOWN', async () => {
        const { fetch } = fakeFetch(
            json({ code: 'FROM_THE_FUTURE' }, 400),
            new Response('Bad Gateway', { status: 418 }),
        );
        const http = client(fetch);
        await expect(http.get('a')).rejects.toMatchObject({ code: 'UNKNOWN', status: 400 });
        await expect(http.get('b')).rejects.toMatchObject({ code: 'UNKNOWN', status: 418 });
    });

    test('reintenta GET ante 503 y errores de red, respetando Retry-After', async () => {
        const waits: number[] = [];
        const { fetch, calls } = fakeFetch(
            json({ code: 'OVERLOADED' }, 503, { 'retry-after': '1' }),
            new TypeError('connection refused'),
            json({ ok: true }),
        );
        const http = client(fetch, { sleep: async (ms: number) => void waits.push(ms) });
        await expect(http.get('health')).resolves.toEqual({ ok: true });
        expect(calls).toHaveLength(3);
        expect(waits[0]).toBe(1000);
        // Todos los intentos comparten request id, para seguirlos en los logs del agente.
        expect(new Set(calls.map((call) => headerOf(call, 'x-request-id'))).size).toBe(1);
    });

    test('nunca reintenta POST: podría duplicar una acción', async () => {
        const { fetch, calls } = fakeFetch(json({ code: 'UNAVAILABLE' }, 503));
        await expect(client(fetch).post('services/nginx/restart')).rejects.toBeInstanceOf(HttpError);
        expect(calls).toHaveLength(1);
    });

    test('agota los reintentos y lanza el último error', async () => {
        const { fetch, calls } = fakeFetch(new TypeError('down'), new TypeError('down'), new TypeError('down'));
        await expect(client(fetch).get('health')).rejects.toBeInstanceOf(NetworkError);
        expect(calls).toHaveLength(3);
    });

    test('distingue tiempo agotado de cancelación del llamador', async () => {
        const fetch = ((_: URL, init: RequestInit) =>
            new Promise<Response>((_resolve, reject) =>
                init.signal?.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError'))),
            )) as unknown as typeof globalThis.fetch;
        const http = client(fetch, { retry: { retries: 0 } });

        await expect(http.get('slow', { timeoutMs: 10 })).rejects.toBeInstanceOf(TimeoutError);

        const controller = new AbortController();
        const cancelled = http.get('slow', { signal: controller.signal, timeoutMs: 5_000 });
        controller.abort();
        await expect(cancelled).rejects.toBeInstanceOf(AbortedError);
    });

    test('parse valida la respuesta', async () => {
        const { fetch } = fakeFetch(
            json({ port: 'no-numero' }),
            new Response('{roto', { headers: { 'content-type': 'application/json' } }),
        );
        const http = client(fetch);
        const parse = (data: unknown) => {
            const port = (data as { port?: unknown }).port;
            if (typeof port !== 'number') throw new TypeError('port debe ser numérico');
            return port;
        };
        await expect(http.get('config', { parse })).rejects.toBeInstanceOf(ResponseFormatError);
        await expect(http.get('config')).rejects.toBeInstanceOf(ResponseFormatError);
    });
});
