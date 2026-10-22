import { beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
    listServers: vi.fn(),
    openTunnel: vi.fn(),
    trustHostKey: vi.fn(),
    closeTunnel: vi.fn(),
    removeServer: vi.fn(),
    forgetHostKey: vi.fn(),
    localAgentStatus: vi.fn(),
    reorderServers: vi.fn(),
    onTunnelClosed: vi.fn(async () => () => {}),
}));

vi.mock('$lib/ssh', async (importOriginal) => ({ ...(await importOriginal<typeof import('$lib/ssh')>()), ...api }));

const { servers } = await import('./servers.svelte');

const PROFILE = {
    id: 'deploy@vps.example.com:22',
    kind: 'ssh' as const,
    host: 'vps.example.com',
    port: 22,
    username: 'deploy',
};
const LOCAL = { id: 'local:4040', kind: 'local' as const, host: 'localhost', port: 4040, username: '' };
const AGENT = { name: 'blacksmith-agent', version: '1.0.0', status: 'ready' as const };
const TUNNEL = { targetId: PROFILE.id, localPort: 51234, remotePort: 4040, agent: AGENT };
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
        expect(api.removeServer).toHaveBeenCalledWith(PROFILE.id);
        expect(servers.entries).toEqual([]);
    });

    describe('servidor local', () => {
        beforeEach(async () => {
            api.listServers.mockResolvedValue([LOCAL]);
            await servers.load();
        });

        test('conecta comprobando el agente, sin túnel', async () => {
            api.localAgentStatus.mockResolvedValue(AGENT);
            await servers.connect(LOCAL.id);
            expect(api.localAgentStatus).toHaveBeenCalledWith(4040);
            expect(api.openTunnel).not.toHaveBeenCalled();
            expect(servers.find(LOCAL.id)).toMatchObject({
                status: 'connected',
                tunnel: { localPort: 4040, agent: AGENT },
            });
        });

        test('sin agente en marcha queda como «agente no configurado»', async () => {
            api.localAgentStatus.mockReturnValue(reject('AGENT_UNREACHABLE'));
            await servers.connect(LOCAL.id);
            expect(servers.find(LOCAL.id)?.status).toBe('agentMissing');
        });

        test('otro servicio en el puerto es un error explicado', async () => {
            api.localAgentStatus.mockReturnValue(reject('AGENT_INVALID'));
            await servers.connect(LOCAL.id);
            expect(servers.find(LOCAL.id)).toMatchObject({ status: 'error', errorCode: 'AGENT_INVALID' });
        });

        test('desconectar no llama a Rust: no hay túnel', async () => {
            api.localAgentStatus.mockResolvedValue(AGENT);
            await servers.connect(LOCAL.id);
            await servers.disconnect(LOCAL.id);
            expect(api.closeTunnel).not.toHaveBeenCalled();
            expect(servers.find(LOCAL.id)?.status).toBe('disconnected');
        });
    });

    describe('orden y vista', () => {
        const B = { ...PROFILE, id: 'deploy@b.example.com:22', host: 'b.example.com' };

        beforeEach(async () => {
            api.listServers.mockResolvedValue([PROFILE, B, LOCAL]);
            await servers.load();
        });

        const ids = () => servers.entries.map((entry) => entry.profile.id);

        test('mueve en local y persiste el orden completo', async () => {
            servers.move(2, 0);
            expect(ids()).toEqual([LOCAL.id, PROFILE.id, B.id]);
            api.reorderServers.mockResolvedValue([]);
            await servers.saveOrder();
            expect(api.reorderServers).toHaveBeenCalledWith([LOCAL.id, PROFILE.id, B.id]);
        });

        test('si Rust rechaza el orden, vuelve al guardado', async () => {
            servers.move(0, 2);
            api.reorderServers.mockReturnValue(reject('INVALID_TARGET'));
            await servers.saveOrder();
            expect(ids()).toEqual([PROFILE.id, B.id, LOCAL.id]);
        });

        test('ignora movimientos fuera de rango', () => {
            servers.move(0, 9);
            servers.move(9, 0);
            expect(ids()).toEqual([PROFILE.id, B.id, LOCAL.id]);
        });

        test('recuerda la vista elegida', () => {
            const stored = new Map<string, string>();
            vi.stubGlobal('localStorage', { setItem: (key: string, value: string) => stored.set(key, value) });
            servers.view = 'grid';
            expect(servers.view).toBe('grid');
            expect(stored.get('blacksmith:servers-view')).toBe('grid');
            servers.view = 'list';
            vi.unstubAllGlobals();
        });
    });
});
