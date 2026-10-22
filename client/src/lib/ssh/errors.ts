/**
 * Errores de los comandos SSH/túnel (Rust: client/src-tauri/src/error.rs). Cada uno trae un `code`
 * estable que la interfaz traduce con `errors.tunnel.<CODE>`. Este módulo no importa nada en tiempo
 * de ejecución, para poder probarlo en Node.
 */

/** Espejo de `CommandError::code()`. Debe mantenerse sincronizado. */
export const TUNNEL_ERROR_CODES = [
    'INVALID_TARGET',
    'KEY_NOT_FOUND',
    'KEY_FILE_UNREADABLE',
    'KEY_TOO_LARGE',
    'KEY_IS_PUBLIC',
    'KEY_INVALID',
    'KEY_UNSUPPORTED',
    'PASSPHRASE_REQUIRED',
    'PASSPHRASE_INVALID',
    'CREDENTIALS_MISSING',
    'KEYRING_UNAVAILABLE',
    'VAULT_UNREADABLE',
    'HOST_UNREACHABLE',
    'CONNECTION_TIMEOUT',
    'HOST_KEY_UNKNOWN',
    'HOST_KEY_CHANGED',
    'HOST_KEY_NOT_PENDING',
    'AUTH_REJECTED',
    'AGENT_UNREACHABLE',
    'AGENT_INVALID',
    'PORT_IN_USE',
    'INTERNAL',
] as const;

export type TunnelErrorCode = (typeof TUNNEL_ERROR_CODES)[number];

const KNOWN = new Set<string>(TUNNEL_ERROR_CODES);

export interface HostKeyDetails {
    algorithm: string;
    /** Huella SHA-256 en formato OpenSSH (`SHA256:…`). */
    fingerprint: string;
}

export class TunnelError extends Error {
    readonly code: TunnelErrorCode;
    /** Huella presentada (`HOST_KEY_UNKNOWN`, `HOST_KEY_CHANGED`). */
    readonly hostKey: HostKeyDetails | undefined;
    /** Puerto local ocupado (`PORT_IN_USE`). */
    readonly port: number | undefined;
    /** Campo inválido del destino (`INVALID_TARGET`): `host`, `port` o `username`. */
    readonly field: 'host' | 'port' | 'username' | undefined;

    constructor(code: TunnelErrorCode, message: string, details: Record<string, unknown> = {}) {
        super(message);
        this.name = 'TunnelError';
        this.code = code;
        const { algorithm, fingerprint, port, field } = details;
        this.hostKey =
            typeof algorithm === 'string' && typeof fingerprint === 'string' ? { algorithm, fingerprint } : undefined;
        this.port = typeof port === 'number' ? port : undefined;
        this.field = field === 'host' || field === 'port' || field === 'username' ? field : undefined;
    }
}

/** Convierte el rechazo de `invoke` (`{ code, message, details? }`) en un `TunnelError`. */
export function toTunnelError(error: unknown): TunnelError {
    if (error instanceof TunnelError) return error;
    if (typeof error === 'object' && error !== null) {
        const { code, message, details } = error as Record<string, unknown>;
        if (typeof code === 'string' && KNOWN.has(code)) {
            return new TunnelError(
                code as TunnelErrorCode,
                typeof message === 'string' ? message : code,
                typeof details === 'object' && details !== null ? (details as Record<string, unknown>) : {},
            );
        }
    }
    return new TunnelError('INTERNAL', error instanceof Error ? error.message : String(error));
}
