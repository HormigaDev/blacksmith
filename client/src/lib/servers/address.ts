/**
 * Cómo se muestra y se enlaza un servidor. Funciones puras: las usan la lista, las tarjetas y la
 * página del servidor.
 */
import type { ServerProfile } from '$lib/ssh';

/** `usuario@host` (con `:puerto` si no es el 22) o `localhost:<puerto>` para este equipo. */
export function serverAddress(profile: ServerProfile): string {
    if (profile.kind === 'local') return `localhost:${profile.port}`;
    return `${profile.username}@${profile.host}${profile.port === 22 ? '' : `:${profile.port}`}`;
}

/**
 * Ruta de la página del servidor. El id lleva `@` y `:` (`deploy@vps.example.com:22`,
 * `local:4040`), así que va codificado; SvelteKit entrega `params.id` ya decodificado.
 */
export function serverHref(id: string): string {
    return `/servers/${encodeURIComponent(id)}`;
}

/** Ruta de la página de un servicio del servidor. */
export function serviceHref(serverId: string, service: string): string {
    return `${serverHref(serverId)}/services/${encodeURIComponent(service)}`;
}
