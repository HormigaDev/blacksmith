/**
 * API del agente de un servidor: servicios registrados y logs en tiempo real. Todo pasa por Rust
 * (`agent_request`, `logs_follow`): ver `transport.ts`.
 */
import { Channel, invoke } from '@tauri-apps/api/core';

import { createHttpClient, type HttpClient } from '$lib/http';
import { toTunnelError } from '$lib/ssh';

import { AGENT_BASE_URL, agentFetch } from './transport';
import {
    type DeployKey,
    type DeployRun,
    type GitHubAccess,
    type EnvVariable,
    type GlobalEnvironment,
    type LogEvent,
    parseDeployRun,
    parseDeployKey,
    parseDeployRuns,
    parseGitHubAccess,
    parseGlobalEnvironment,
    parseLogEvent,
    parseServiceDeployment,
    parseServiceEnvironment,
    parseServiceList,
    parseServiceStatus,
    parseSite,
    parseServiceSummary,
    type RegisterService,
    type ServiceAction,
    type ServiceDeployment,
    type ServiceEnvironment,
    type ServiceStatus,
    type ServiceSummary,
    type Site,
    type SiteConfig,
    type UpdateDeployment,
} from './types';

export function agentClient(serverId: string): HttpClient {
    return createHttpClient({ baseUrl: AGENT_BASE_URL, fetch: agentFetch(serverId) });
}

export function listServices(client: HttpClient, signal?: AbortSignal): Promise<ServiceSummary[]> {
    return client.get('services', { parse: parseServiceList, signal });
}

export function registerService(client: HttpClient, service: RegisterService): Promise<ServiceSummary> {
    return client.post('services', { body: service, parse: parseServiceSummary });
}

/** Quita el servicio del registro y borra sus variables de entorno. No toca systemd. */
export async function unregisterService(client: HttpClient, name: string): Promise<void> {
    await client.delete(`services/${encodeURIComponent(name)}`);
}

const servicePath = (name: string, rest = '') => `services/${encodeURIComponent(name)}${rest}`;

export function serviceStatus(client: HttpClient, name: string, signal?: AbortSignal): Promise<ServiceStatus> {
    return client.get(servicePath(name, '/status'), { parse: parseServiceStatus, signal });
}

/** Arranca, para o reinicia la unidad. Resuelve cuando systemd terminó el trabajo. */
export async function serviceAction(client: HttpClient, name: string, action: ServiceAction): Promise<void> {
    // Las acciones pueden tardar (el agente espera a systemd): más margen que el habitual.
    await client.post(servicePath(name, `/${action}`), { timeoutMs: 120_000 });
}

export function serviceEnvironment(client: HttpClient, name: string): Promise<ServiceEnvironment> {
    return client.get(servicePath(name, '/env'), { parse: parseServiceEnvironment });
}

export function saveServiceEnvironment(
    client: HttpClient,
    name: string,
    variables: EnvVariable[],
): Promise<ServiceEnvironment> {
    return client.put(servicePath(name, '/env'), { body: { variables }, parse: parseServiceEnvironment });
}

export function globalEnvironment(client: HttpClient): Promise<GlobalEnvironment> {
    return client.get('env', { parse: parseGlobalEnvironment });
}

export function saveGlobalEnvironment(client: HttpClient, variables: EnvVariable[]): Promise<GlobalEnvironment> {
    return client.put('env', { body: { variables }, parse: parseGlobalEnvironment });
}

export function serviceDeployment(client: HttpClient, name: string): Promise<ServiceDeployment> {
    return client.get(servicePath(name, '/deployment'), { parse: parseServiceDeployment });
}

export function saveServiceDeployment(
    client: HttpClient,
    name: string,
    deployment: UpdateDeployment,
): Promise<ServiceDeployment> {
    return client.put(servicePath(name, '/deployment'), { body: deployment, parse: parseServiceDeployment });
}

/** Llave de despliegue del servicio (parte pública), o `null` si aún no se generó. */
export function deployKey(client: HttpClient, name: string): Promise<DeployKey | null> {
    return client.get(servicePath(name, '/deploy-key'), { parse: parseDeployKey });
}

/** Genera la llave de despliegue en el servidor si no existe (idempotente). */
export async function createDeployKey(client: HttpClient, name: string): Promise<DeployKey> {
    const key = await client.post(servicePath(name, '/deploy-key'), { parse: parseDeployKey });
    if (!key) throw new TypeError('respuesta del agente inesperada: llave vacía');
    return key;
}

/** Sitio de nginx del servicio, o `null` si no tiene. */
export function serviceSite(client: HttpClient, name: string): Promise<Site | null> {
    return client.get(servicePath(name, '/site'), { parse: parseSite });
}

/** Guarda el sitio y lo aplica en nginx (si nginx lo rechaza, sigue el anterior). */
export async function saveServiceSite(client: HttpClient, name: string, config: SiteConfig): Promise<Site> {
    const site = await client.put(servicePath(name, '/site'), { body: config, parse: parseSite });
    if (!site) throw new TypeError('respuesta del agente inesperada: sitio vacío');
    return site;
}

export async function removeServiceSite(client: HttpClient, name: string): Promise<void> {
    await client.delete(servicePath(name, '/site'));
}

/** Vuelve a pedir el certificado de Let's Encrypt. */
export async function retryCertificate(client: HttpClient, name: string): Promise<Site> {
    const site = await client.post(servicePath(name, '/site/certificate'), { parse: parseSite });
    if (!site) throw new TypeError('respuesta del agente inesperada: sitio vacío');
    return site;
}

/** Cuenta de GitHub del servidor (nunca el token), o `null` si no tiene. */
export function githubAccess(client: HttpClient): Promise<GitHubAccess | null> {
    return client.get('github', { parse: parseGitHubAccess });
}

/**
 * Deja en el servidor la cuenta de GitHub conectada en este equipo, para que descargue
 * repositorios privados por su cuenta. El token va de Rust al agente: nunca pasa por aquí.
 */
export async function shareGitHubWithServer(serverId: string): Promise<GitHubAccess> {
    try {
        const access = parseGitHubAccess(await invoke('github_share_with_server', { serverId }));
        if (!access) throw new TypeError('respuesta del agente inesperada: acceso vacío');
        return access;
    } catch (error) {
        throw toTunnelError(error);
    }
}

/** Vuelve a la versión anterior; sigue en segundo plano, como un despliegue. */
export function rollback(client: HttpClient, name: string): Promise<DeployRun> {
    return client.post(servicePath(name, '/rollback'), { parse: parseDeployRun });
}

/** Lanza un despliegue; sigue en segundo plano (ver `deployRuns` y los logs del despliegue). */
export function deploy(client: HttpClient, name: string): Promise<DeployRun> {
    return client.post(servicePath(name, '/deploy'), { parse: parseDeployRun });
}

export function deployRuns(client: HttpClient, name: string, signal?: AbortSignal): Promise<DeployRun[]> {
    return client.get(servicePath(name, '/deploys'), { parse: parseDeployRuns, signal });
}

/**
 * Sigue los logs del servicio: primero las últimas entradas y luego las nuevas. Devuelve la
 * función para dejar de seguirlos. Si el agente rechaza la conexión, falla con un `TunnelError`
 * (`AGENT_REJECTED`, con el código del agente).
 */
export async function followLogs(
    serverId: string,
    service: string,
    onEvent: (event: LogEvent) => void,
    /** `deploy`: la salida de los despliegues; `since` (segundos Unix): solo desde ese instante. */
    options: { deploy?: boolean; since?: number } = {},
): Promise<() => Promise<void>> {
    const channel = new Channel<unknown>();
    channel.onmessage = (message) => {
        try {
            onEvent(parseLogEvent(message));
        } catch (error) {
            console.warn('Evento de log descartado', error);
        }
    };
    let id: number;
    try {
        id = await invoke<number>('logs_follow', {
            serverId,
            service,
            deploy: options.deploy ?? false,
            since: options.since ?? null,
            onEvent: channel,
        });
    } catch (error) {
        throw toTunnelError(error);
    }
    return async () => {
        await invoke('logs_stop', { id }).catch(() => undefined);
    };
}
