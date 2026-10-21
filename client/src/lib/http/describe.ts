import { t } from '$lib/i18n';
import { TunnelError } from '$lib/ssh/errors';

import { AbortedError, HttpError, NetworkError, ResponseFormatError, TimeoutError } from './client';

/**
 * Mensaje traducido para mostrar al usuario a partir de cualquier error capturado.
 * Los errores del agente se traducen por su `code` (`errors.agent.<CODE>`), nunca por su
 * `message`. Si se añade un código en `shared::ErrorCode` sin su clave, no compila. Lo mismo con
 * los del túnel SSH (`errors.tunnel.<CODE>`).
 */
export function describeError(error: unknown): string {
    if (error instanceof TunnelError) return t(`errors.tunnel.${error.code}`, { port: error.port ?? 0 });
    if (error instanceof HttpError) return t(`errors.agent.${error.code}`);
    if (error instanceof TimeoutError) return t('errors.timeout');
    if (error instanceof NetworkError) return t('errors.network');
    if (error instanceof ResponseFormatError || error instanceof AbortedError) return t('errors.unexpected');
    return t('errors.unexpected');
}
