import { t } from '$lib/i18n';

import { AbortedError, HttpError, NetworkError, ResponseFormatError, TimeoutError } from './client';

/**
 * Mensaje traducido para mostrar al usuario a partir de cualquier error capturado.
 * Los errores del agente se traducen por su `code` (`errors.agent.<CODE>`), nunca por su
 * `message`. Si se añade un código en `shared::ErrorCode` sin su clave, no compila.
 */
export function describeError(error: unknown): string {
    if (error instanceof HttpError) return t(`errors.agent.${error.code}`);
    if (error instanceof TimeoutError) return t('errors.timeout');
    if (error instanceof NetworkError) return t('errors.network');
    if (error instanceof ResponseFormatError || error instanceof AbortedError) return t('errors.unexpected');
    return t('errors.unexpected');
}
