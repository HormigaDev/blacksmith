import { beforeEach, describe, expect, test, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

const { agentFetch, AGENT_BASE_URL } = await import('./transport');
const { createHttpClient, HttpError, NetworkError, TimeoutError, describeError } = await import('$lib/http');

const json = (status: number, body: unknown) => ({
    status,
    contentType: 'application/json',
    retryAfter: null,
    requestId: 'r1',
    body: JSON.stringify(body),
});

describe('agentFetch', () => {
    beforeEach(() => {
        invoke.mockReset();
    });

    test('el tiempo máximo corta la espera aunque Rust no responda', async () => {
        invoke.mockReturnValue(new Promise(() => {}));
        const client = createHttpClient({
            baseUrl: AGENT_BASE_URL,
            fetch: agentFetch('deploy@vps:22'),
            retry: { retries: 0 },
        });
        await expect(client.get('services', { timeoutMs: 20 })).rejects.toBeInstanceOf(TimeoutError);
    });

    test('envía a Rust solo el servidor, el método, la ruta y el cuerpo', async () => {
        invoke.mockResolvedValue(json(201, { ok: true }));
        const client = createHttpClient({ baseUrl: AGENT_BASE_URL, fetch: agentFetch('deploy@vps:22') });
        await expect(client.post('services', { query: { x: 1 }, body: { name: 'api' } })).resolves.toEqual({
            ok: true,
        });
        const [command, args] = invoke.mock.calls[0]!;
        expect(command).toBe('agent_request');
        expect(args).toMatchObject({
            request: {
                serverId: 'deploy@vps:22',
                method: 'POST',
                path: '/api/v1/services?x=1',
                body: '{"name":"api"}',
            },
        });
        expect((args as { request: { requestId: string } }).request.requestId).toMatch(/^[a-z0-9-]+$/i);
    });

    test('los errores del agente llegan como HttpError con su código', async () => {
        invoke.mockResolvedValue(json(409, { code: 'SERVICE_ALREADY_EXISTS', message: 'repetido' }));
        const client = createHttpClient({ baseUrl: AGENT_BASE_URL, fetch: agentFetch('s') });
        const error = await client.post('services', { body: {} }).catch((error: unknown) => error);
        expect(error).toBeInstanceOf(HttpError);
        expect((error as InstanceType<typeof HttpError>).code).toBe('SERVICE_ALREADY_EXISTS');
    });

    test('204 sin cuerpo', async () => {
        invoke.mockResolvedValue({ status: 204, contentType: null, retryAfter: null, requestId: null, body: '' });
        const client = createHttpClient({ baseUrl: AGENT_BASE_URL, fetch: agentFetch('s') });
        await expect(client.delete('services/api')).resolves.toBeUndefined();
    });

    test('servidor desconectado: error de red con el motivo del túnel', async () => {
        invoke.mockRejectedValue({ code: 'NOT_CONNECTED', message: 'no conectado' });
        const client = createHttpClient({
            baseUrl: AGENT_BASE_URL,
            fetch: agentFetch('s'),
            retry: { retries: 0 },
        });
        const error = await client.get('services').catch((error: unknown) => error);
        expect(error).toBeInstanceOf(NetworkError);
        expect(describeError(error)).toBe('Conéctate al servidor antes de continuar.');
    });
});
