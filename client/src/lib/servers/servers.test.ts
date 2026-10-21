import { beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
    listServers: vi.fn(),
    openTunnel: vi.fn(),
    trustHostKey: vi.fn(),
    closeTunnel: vi.fn(),
    removeServer: vi.fn(),
    forgetHostKey: vi.fn(),
    onTunnelClosed: vi.fn(async () => () => {}),
}));

vi.mock('$lib/ssh', async (importOriginal) => ({ ...(await importOriginal<typeof import('$lib/ssh')>()), ...api }));

const { servers } = await import('./servers.svelte');

const PROFILE = { id: 'deploy@vps.example.com:22', host: 'vps.example.com', port: 22, username: 'deploy' };
const TUNNEL = { targetId: PROFILE.id, localPort: 51234, remotePort: 4040 };
const HOST_KEY = { algorithm: 'ssh-ed25519', fingerprint: 'SHA256:abc' };
const reject = (code: string, details?: object) => Promise.reject({ code, message: code, details });

describe('servers', () => {
    beforeEach(async () => {
        vi.clearAllMocks();
        api.listServers.mockResolvedValue([PROFILE]);
        await servers.load();
        servers.pendingTrust = null;
        servers.pendingPassphrase = null;
    });

    test('lista los servidores guardados, sin conectar', () => {
        expect(servers.entries).toEqual([{ profile: PROFILE, status: 'disconnected' }]);
    });

    test('conecta con el destino del servidor', async () => {
        api.openTunnel.mockResolvedValue(TUNNEL);
        await servers.connect(PROFILE.id);
        expect(api.openTunnel).toHaveBeenCalledWith({ host: 'vps.example.com', port: 22, username: 'deploy' }, {});
        expect(servers.find(PROFILE.id)).toMatchObject({ status: 'connected', tunnel: TUNNEL });
    });

    test('si el agente no responde, el servidor se conserva como «agente no configurado»', async () => {
        api.openTunnel.mockReturnValue(reject('AGENT_UNREACHABLE'));
        await servers.connect(PROFILE.id);
        expect(servers.find(PROFILE.id)?.status).toBe('agentMissing');
        expect(servers.entries).toHaveLength(1);

        // Reintentar no pide nada otra vez.
        api.openTunnel.mockResolvedValue(TUNNEL);
        await servers.connect(PROFILE.id);
        expect(servers.find(PROFILE.id)?.status).toBe('connected');
    });

    test('un servidor nuevo queda pendiente de confirmar su huella', async () => {
        api.openTunnel.mockReturnValueOnce(reject('HOST_KEY_UNKNOWN', HOST_KEY)).mockResolvedValueOnce(TUNNEL);
        await servers.connect(PROFILE.id);
        expect(servers.pendingTrust).toEqual({ id: PROFILE.id, hostKey: HOST_KEY });
        expect(servers.find(PROFILE.id)?.status).toBe('disconnected');

        await servers.trust();
        expect(api.trustHostKey).toHaveBeenCalledWith(
            expect.objectContaining({ host: 'vps.example.com' }),
            'SHA256:abc',
        );
        expect(servers.pendingTrust).toBeNull();
        expect(servers.find(PROFILE.id)?.status).toBe('connected');
    });

    test('pide la passphrase y distingue si la anterior era incorrecta', async () => {
        api.openTunnel.mockReturnValueOnce(reject('PASSPHRASE_REQUIRED'));
        await servers.connect(PROFILE.id);
        expect(servers.pendingPassphrase).toEqual({ id: PROFILE.id, invalid: false });

        api.openTunnel.mockReturnValueOnce(reject('PASSPHRASE_INVALID'));
        await servers.connect(PROFILE.id, { passphrase: 'mal' });
        expect(servers.pendingPassphrase).toEqual({ id: PROFILE.id, invalid: true });
    });

    test('otros fallos quedan en el servidor con su mensaje traducido', async () => {
        api.openTunnel.mockReturnValue(reject('HOST_UNREACHABLE'));
        await servers.connect(PROFILE.id);
        expect(servers.find(PROFILE.id)).toMatchObject({
            status: 'error',
            error: 'No se pudo contactar con el servidor. Revisa el host, el puerto y tu conexión.',
        });
    });

    test('una llave de servidor cambiada se puede olvidar explícitamente', async () => {
        api.openTunnel.mockReturnValue(reject('HOST_KEY_CHANGED', HOST_KEY));
        await servers.connect(PROFILE.id);
        expect(servers.find(PROFILE.id)).toMatchObject({ status: 'error', errorCode: 'HOST_KEY_CHANGED' });
        expect(api.trustHostKey).not.toHaveBeenCalled();

        api.forgetHostKey.mockResolvedValue(true);
        await servers.forgetHostKey(PROFILE.id);
        expect(servers.find(PROFILE.id)).toMatchObject({ status: 'disconnected', errorCode: undefined });
    });

    test('quitar un servidor lo borra en Rust y de la lista', async () => {
        api.removeServer.mockResolvedValue(true);
        await servers.remove(PROFILE.id);
        expect(api.removeServer).toHaveBeenCalledWith({ host: 'vps.example.com', port: 22, username: 'deploy' });
        expect(servers.entries).toEqual([]);
    });
});
