/**
 * Servidores configurados y su estado de conexión. Única fuente de verdad: el dashboard, el
 * formulario de alta y las tarjetas leen y actúan a través de `servers`.
 *
 * Un servidor se guarda en cuanto su llave es válida, aunque luego no se pueda conectar: si el
 * agente no responde, el túnel se cierra y el servidor queda listado como «agente no configurado»,
 * listo para reintentar sin volver a introducir nada.
 */
import { describeError } from '$lib/http';
import { t } from '$lib/i18n';
import {
    closeTunnel,
    forgetHostKey,
    type HostKeyDetails,
    listServers,
    onTunnelClosed,
    openTunnel,
    removeServer,
    type ServerProfile,
    targetOf,
    toTunnelError,
    trustHostKey,
    type TunnelErrorCode,
    type TunnelInfo,
} from '$lib/ssh';
import { toast } from '$lib/toast';

export type ServerStatus = 'disconnected' | 'connecting' | 'connected' | 'agentMissing' | 'error';

export interface ServerEntry {
    profile: ServerProfile;
    status: ServerStatus;
    tunnel?: TunnelInfo;
    /** Mensaje ya traducido cuando `status` es `error`. */
    error?: string;
    /** Código del fallo: p. ej. `HOST_KEY_CHANGED` habilita olvidar la llave registrada. */
    errorCode?: TunnelErrorCode;
}

class ServersState {
    #entries = $state<ServerEntry[]>([]);
    #loaded = $state(false);
    /** Servidor nuevo: el usuario debe confirmar su huella antes de conectar. */
    pendingTrust = $state<{ id: string; hostKey: HostKeyDetails } | null>(null);
    /** La llave necesita passphrase (no se guardó, o la guardada ya no vale). */
    pendingPassphrase = $state<{ id: string; invalid: boolean } | null>(null);

    get entries(): readonly ServerEntry[] {
        return this.#entries;
    }

    get loaded(): boolean {
        return this.#loaded;
    }

    find(id: string): ServerEntry | undefined {
        return this.#entries.find((entry) => entry.profile.id === id);
    }

    async load(): Promise<void> {
        const profiles = await listServers();
        // Conserva el estado de los que ya se conocían (p. ej. un túnel abierto).
        this.#entries = profiles.map((profile) => this.find(profile.id) ?? { profile, status: 'disconnected' });
        this.#loaded = true;
    }

    /** Tras darlo de alta con el formulario. */
    added(profile: ServerProfile): void {
        if (!this.find(profile.id)) this.#entries = [...this.#entries, { profile, status: 'disconnected' }];
    }

    async connect(id: string, options: { passphrase?: string } = {}): Promise<void> {
        const entry = this.find(id);
        if (!entry || entry.status === 'connecting') return;
        this.#update(id, { status: 'connecting', error: undefined, errorCode: undefined });
        try {
            const tunnel = await openTunnel(targetOf(entry.profile), options);
            this.#update(id, { status: 'connected', tunnel });
            toast.success(t('servers.connect.connected', { host: entry.profile.host }), {
                description: t('servers.connect.connectedDetail', { port: tunnel.localPort }),
            });
        } catch (error) {
            this.#failed(id, error);
        }
    }

    /** El usuario confirmó la huella presentada. */
    async trust(): Promise<void> {
        const pending = this.pendingTrust;
        const entry = pending && this.find(pending.id);
        this.pendingTrust = null;
        if (!pending || !entry) return;
        try {
            await trustHostKey(targetOf(entry.profile), pending.hostKey.fingerprint);
        } catch (error) {
            this.#failed(pending.id, error);
            return;
        }
        await this.connect(pending.id);
    }

    async disconnect(id: string): Promise<void> {
        const entry = this.find(id);
        if (!entry) return;
        try {
            await closeTunnel(targetOf(entry.profile));
            this.#update(id, { status: 'disconnected', tunnel: undefined });
        } catch (error) {
            toast.error(describeError(error));
        }
    }

    /**
     * Olvida la llave registrada del servidor tras un `HOST_KEY_CHANGED` (p. ej. se reinstaló).
     * Acción destructiva: la interfaz la confirma antes. Al reconectar se pedirá la nueva huella.
     */
    async forgetHostKey(id: string): Promise<void> {
        const entry = this.find(id);
        if (!entry) return;
        try {
            await forgetHostKey(targetOf(entry.profile));
            this.#update(id, { status: 'disconnected', error: undefined, errorCode: undefined });
        } catch (error) {
            toast.error(describeError(error));
        }
    }

    /** Acción destructiva: la interfaz la confirma antes. */
    async remove(id: string): Promise<void> {
        const entry = this.find(id);
        if (!entry) return;
        try {
            await removeServer(targetOf(entry.profile));
            this.#entries = this.#entries.filter((candidate) => candidate.profile.id !== id);
        } catch (error) {
            toast.error(describeError(error));
        }
    }

    /** Escucha cierres y pérdidas de túneles. Devuelve la función de limpieza. */
    watch(): () => void {
        let stop: (() => void) | undefined;
        let disposed = false;
        void onTunnelClosed((event) => {
            const entry = this.find(event.targetId);
            if (!entry || entry.status !== 'connected') return;
            this.#update(event.targetId, { status: 'disconnected', tunnel: undefined });
            if (event.reason === 'connectionLost') {
                toast.warning(t('servers.connect.lost', { host: entry.profile.host }));
            }
        }).then((unlisten) => {
            if (disposed) unlisten();
            else stop = unlisten;
        });
        return () => {
            disposed = true;
            stop?.();
        };
    }

    #failed(id: string, error: unknown): void {
        const failure = toTunnelError(error);
        switch (failure.code) {
            case 'HOST_KEY_UNKNOWN':
                this.#update(id, { status: 'disconnected' });
                if (failure.hostKey) this.pendingTrust = { id, hostKey: failure.hostKey };
                break;
            case 'PASSPHRASE_REQUIRED':
            case 'PASSPHRASE_INVALID':
                this.#update(id, { status: 'disconnected' });
                this.pendingPassphrase = { id, invalid: failure.code === 'PASSPHRASE_INVALID' };
                break;
            case 'AGENT_UNREACHABLE':
                // SSH funciona pero el agente no responde: aún no está instalado o configurado.
                this.#update(id, { status: 'agentMissing' });
                break;
            default:
                this.#update(id, { status: 'error', error: describeError(failure), errorCode: failure.code });
        }
    }

    #update(id: string, changes: Partial<ServerEntry>): void {
        this.#entries = this.#entries.map((entry) => (entry.profile.id === id ? { ...entry, ...changes } : entry));
    }
}

export const servers = new ServersState();
