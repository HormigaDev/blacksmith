import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import {
    AbortedError,
    createHttpClient,
    HttpError,
    NetworkError,
    ResponseFormatError,
    resolveUrl,
    TimeoutError,
} from './client.ts';

type Call = { url: string; init: RequestInit };

/** Transporte falso: responde en orden con las respuestas (o errores) dados. */
function fakeFetch(...replies: (Response | Error | (() => Promise<Response>))[]) {
    const calls: Call[] = [];
    const fetch = (async (url: URL, init: RequestInit) => {
        calls.push({ url: String(url), init });
        const reply = replies.shift();
        if (reply === undefined) throw new Error('sin más respuestas');
        if (reply instanceof Error) throw reply;
        return typeof reply === 'function' ? reply() : reply;
    }) as unknown as typeof globalThis.fetch;
    return { fetch, calls };
}

const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
    new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', ...headers } });

const noSleep = async () => {};
const client = (fetch: typeof globalThis.fetch, extra = {}) =>
    createHttpClient({ baseUrl: 'http://127.0.0.1:4040/api/v1', fetch, sleep: noSleep, ...extra });

describe('resolveUrl', () => {
    const base = new URL('http://127.0.0.1:4040/api/v1/');

    test('resuelve rutas relativas y serializa la query', () => {
        const url = resolveUrl(base, '/services/nginx', { lines: 50, unit: ['a', 'b'], skip: undefined, none: null });
        assert.equal(url.href, 'http://127.0.0.1:4040/api/v1/services/nginx?lines=50&unit=a&unit=b');
    });

    test('nunca sale del origen ni de la base', () => {
        for (const path of ['https://evil.example/x', '//evil.example/x', 'javascript:alert(1)', '../../admin', '..']) {
            assert.throws(() => resolveUrl(base, path), TypeError, path);
        }
    });
});

describe('createHttpClient', () => {
    test('GET devuelve el JSON y envía cabeceras de control', async () => {
        const { fetch, calls } = fakeFetch(json({ status: 'ready' }));
        assert.deepEqual(await client(fetch).get('health'), { status: 'ready' });
        const headers = new Headers(calls[0]?.init.headers);
        assert.equal(headers.get('accept'), 'application/json');
        assert.match(headers.get('x-request-id') ?? '', /^[0-9a-f-]{36}$/);
        assert.equal(calls[0]?.init.redirect, 'error');
    });

    test('POST serializa objetos como JSON', async () => {
        const { fetch, calls } = fakeFetch(new Response(null, { status: 204 }));
        assert.equal(await client(fetch).post('deploy', { body: { branch: 'main' } }), undefined);
        assert.equal(calls[0]?.init.body, '{"branch":"main"}');
        assert.equal(new Headers(calls[0]?.init.headers).get('content-type'), 'application/json');
    });

    test('los errores del agente conservan su código estable', async () => {
        const { fetch } = fakeFetch(json({ code: 'NOT_FOUND', message: 'recurso no encontrado' }, 404));
        await assert.rejects(client(fetch).get('services/x'), (error: unknown) => {
            assert.ok(error instanceof HttpError);
            assert.equal(error.status, 404);
            assert.equal(error.code, 'NOT_FOUND');
            assert.equal(error.detail, 'recurso no encontrado');
            return true;
        });
    });

    test('un código desconocido o un cuerpo no JSON es UNKNOWN', async () => {
        const { fetch } = fakeFetch(
            json({ code: 'FROM_THE_FUTURE' }, 400),
            new Response('Bad Gateway', { status: 418 }),
        );
        const http = client(fetch);
        await assert.rejects(http.get('a'), (error: unknown) => error instanceof HttpError && error.code === 'UNKNOWN');
        await assert.rejects(http.get('b'), (error: unknown) => error instanceof HttpError && error.code === 'UNKNOWN');
    });

    test('reintenta GET ante 503 y errores de red, respetando Retry-After', async () => {
        const waits: number[] = [];
        const { fetch, calls } = fakeFetch(
            json({ code: 'OVERLOADED' }, 503, { 'retry-after': '1' }),
            new TypeError('connection refused'),
            json({ ok: true }),
        );
        const http = client(fetch, { sleep: async (ms: number) => void waits.push(ms) });
        assert.deepEqual(await http.get('health'), { ok: true });
        assert.equal(calls.length, 3);
        assert.equal(waits[0], 1000);
        const ids = calls.map((call) => new Headers(call.init.headers).get('x-request-id'));
        assert.equal(new Set(ids).size, 1, 'todos los intentos comparten request id');
    });

    test('nunca reintenta POST: podría duplicar una acción', async () => {
        const { fetch, calls } = fakeFetch(json({ code: 'UNAVAILABLE' }, 503));
        await assert.rejects(client(fetch).post('services/nginx/restart'), HttpError);
        assert.equal(calls.length, 1);
    });

    test('agota los reintentos y lanza el último error', async () => {
        const { fetch, calls } = fakeFetch(new TypeError('down'), new TypeError('down'), new TypeError('down'));
        await assert.rejects(client(fetch).get('health'), NetworkError);
        assert.equal(calls.length, 3);
    });

    test('distingue tiempo agotado de cancelación del llamador', async () => {
        const hang = (init?: RequestInit) =>
            new Promise<Response>((_, reject) =>
                init?.signal?.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError'))),
            );
        const fetch = ((_: URL, init: RequestInit) => hang(init)) as unknown as typeof globalThis.fetch;
        const http = client(fetch, { retry: { retries: 0 } });

        await assert.rejects(http.get('slow', { timeoutMs: 10 }), TimeoutError);

        const controller = new AbortController();
        const cancelled = http.get('slow', { signal: controller.signal, timeoutMs: 5_000 });
        controller.abort();
        await assert.rejects(cancelled, AbortedError);
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
        await assert.rejects(http.get('config', { parse }), ResponseFormatError);
        await assert.rejects(http.get('config'), ResponseFormatError);
    });
});
