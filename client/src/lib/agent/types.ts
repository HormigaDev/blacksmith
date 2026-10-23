/**
 * Espejo de los tipos de la API del agente (crate `shared`). Este módulo no importa nada en tiempo
 * de ejecución, para poder probarlo en Node. Los `parse*` validan lo que llega: es una frontera de
 * confianza.
 */

/** `shared::ServiceSummary`. */
export interface ServiceSummary {
    name: string;
    displayName: string;
    description: string;
    unit: string;
    /** Segundos Unix. */
    createdAt: number;
}

/** `shared::RegisterService`. */
export interface RegisterService {
    name: string;
    displayName: string;
    description: string;
}

/** `shared::LogEntry`. */
export interface LogEntry {
    timestampUs: number;
    priority: number | null;
    pid: number | null;
    identifier: string | null;
    message: string;
}

export const LOG_END_REASONS = ['journalClosed', 'agentShutdown', 'clientTooSlow', 'connectionLost'] as const;
export type LogEndReason = (typeof LOG_END_REASONS)[number];

/** `shared::LogEvent`. */
export type LogEvent = ({ type: 'entry' } & LogEntry) | { type: 'end'; reason: LogEndReason; detail: string | null };

/** `shared::EnvVariable`. */
export interface EnvVariable {
    key: string;
    value: string;
}

/** `shared::ServiceEnvironment`. */
export interface ServiceEnvironment {
    name: string;
    unit: string;
    file: string;
    loadedByUnit: boolean;
    globalLoadedByUnit: boolean;
    variables: EnvVariable[];
}

/** `shared::GlobalEnvironment`. */
export interface GlobalEnvironment {
    file: string;
    variables: EnvVariable[];
}

/** `shared::RepositoryRef`. */
export interface RepositoryRef {
    owner: string;
    name: string;
    branch: string;
}

/** `shared::ServiceDeployment`. */
/**
 * `shared::RepositoryAccess`: cómo descarga el servidor el repositorio. `account`: por HTTPS, con
 * la cuenta de GitHub del servidor si es privado. `deployKey`: por SSH, con una llave de solo
 * lectura de ese repositorio.
 */
export type RepositoryAccess = 'account' | 'deployKey';

export interface ServiceDeployment {
    name: string;
    repository: RepositoryRef | null;
    access: RepositoryAccess;
    workingDirectory: string;
    setupCommands: string[];
    startCommand: string;
    /** Carpeta del repositorio que se publica para nginx (sitios estáticos), p. ej. `dist`. */
    publishDir: string | null;
    updatedAt: number | null;
}

/** `shared::UpdateDeployment`. */
export interface UpdateDeployment {
    repository: RepositoryRef | null;
    access: RepositoryAccess;
    setupCommands: string[];
    startCommand: string;
    publishDir: string | null;
}

/** `shared::SiteTemplate`: tipo de proyecto (plantilla de nginx). */
export const SITE_TEMPLATES = ['static', 'spa', 'app', 'api', 'websocket', 'fullstack'] as const;
export type SiteTemplate = (typeof SITE_TEMPLATES)[number];

/** Plantillas que sirven la carpeta pública del despliegue. */
export const servesFiles = (template: SiteTemplate): boolean =>
    template === 'static' || template === 'spa' || template === 'fullstack';
/** Plantillas que reenvían al servicio (necesitan su puerto). */
export const proxies = (template: SiteTemplate): boolean =>
    template === 'app' || template === 'api' || template === 'websocket' || template === 'fullstack';

/** `shared::SiteConfig`. */
export interface SiteConfig {
    template: SiteTemplate;
    /** El primero es el principal. */
    domains: string[];
    port: number | null;
    https: boolean;
    email: string | null;
    /** Rutas hacia otros puertos, antes que la plantilla (p. ej. `/api/` → 3000). */
    routes: SiteRoute[];
}

/** `shared::SiteRoute`: prefijo de ruta que nginx reenvía a otro puerto local. */
export interface SiteRoute {
    path: string;
    port: number;
    websocket: boolean;
}

const isSiteRoute = (value: unknown): value is SiteRoute =>
    isRecord(value) &&
    typeof value.path === 'string' &&
    typeof value.port === 'number' &&
    typeof value.websocket === 'boolean';

export const TLS_STATUSES = ['off', 'issuing', 'active', 'failed'] as const;
export type TlsStatus = (typeof TLS_STATUSES)[number];

/** `shared::Site`. */
export interface Site extends SiteConfig {
    name: string;
    tls: TlsStatus;
    tlsDetail: string | null;
    configPath: string;
    updatedAt: number;
}

export function parseSite(data: unknown): Site | null {
    if (data === null) return null;
    if (!isRecord(data)) fail('sitio');
    const { name, domains, port, https, email, tlsDetail, configPath, updatedAt } = data;
    // Un agente anterior a las rutas no envía el campo.
    const routes = data.routes ?? [];
    const template = SITE_TEMPLATES.find((known) => known === data.template);
    const tls = TLS_STATUSES.find((known) => known === data.tls);
    if (
        !template ||
        !tls ||
        typeof name !== 'string' ||
        !Array.isArray(domains) ||
        !domains.every(isString) ||
        !(port === null || typeof port === 'number') ||
        typeof https !== 'boolean' ||
        !(email === null || typeof email === 'string') ||
        !Array.isArray(routes) ||
        !routes.every(isSiteRoute) ||
        !(tlsDetail === null || typeof tlsDetail === 'string') ||
        typeof configPath !== 'string' ||
        typeof updatedAt !== 'number'
    ) {
        fail('sitio');
    }
    return { name, template, domains, port, https, email, routes, tls, tlsDetail, configPath, updatedAt };
}

export const LOAD_STATES = ['loaded', 'not-found', 'bad-setting', 'error', 'masked', 'merged', 'stub'] as const;
export const ACTIVE_STATES = [
    'active',
    'reloading',
    'inactive',
    'failed',
    'activating',
    'deactivating',
    'maintenance',
    'refreshing',
] as const;

/** Acciones de ciclo de vida (`POST /services/{name}/{acción}`). */
export type ServiceAction = 'start' | 'stop' | 'restart';

/** `shared::ServiceStatusReport`. */
export interface ServiceStatus {
    name: string;
    unit: string;
    loadState: (typeof LOAD_STATES)[number];
    activeState: (typeof ACTIVE_STATES)[number];
    subState: string;
    pid: number | null;
    metrics: { rssBytes: number; rssMb: number; cpuPercent: number } | null;
    /** Hay una versión activa; sin ella el servicio no puede arrancar. */
    deployed: boolean;
    /** Versión activa y la anterior (a la que vuelve un rollback). */
    release: string | null;
    previousRelease: string | null;
    /** Consumo del servicio entero (su cgroup: todos sus procesos). */
    resources: ServiceResources;
    /** Capacidad del servidor. */
    host: HostCapacity;
}

/** `shared::ServiceResources`. `null` si no se conoce (parado, sin contabilidad o sin límite). */
export interface ServiceResources {
    memoryBytes: number | null;
    memoryPeakBytes: number | null;
    memoryLimitBytes: number | null;
    swapBytes: number | null;
    /** 100 = un núcleo completo. */
    cpuPercent: number | null;
    tasks: number | null;
    restarts: number | null;
    uptimeSecs: number | null;
}

/** `shared::HostCapacity`. */
export interface HostCapacity {
    cpuCores: number;
    memoryBytes: number;
    swapBytes: number;
}

const RESOURCE_KEYS = [
    'memoryBytes',
    'memoryPeakBytes',
    'memoryLimitBytes',
    'swapBytes',
    'cpuPercent',
    'tasks',
    'restarts',
    'uptimeSecs',
] as const;

function parseResources(data: unknown): ServiceResources {
    if (!isRecord(data)) fail('consumo');
    const resources = {} as Record<(typeof RESOURCE_KEYS)[number], number | null>;
    for (const key of RESOURCE_KEYS) {
        const value = data[key];
        if (!(value === null || typeof value === 'number')) fail('consumo');
        resources[key] = value;
    }
    return resources;
}

function parseHost(data: unknown): HostCapacity {
    if (
        !isRecord(data) ||
        typeof data.cpuCores !== 'number' ||
        typeof data.memoryBytes !== 'number' ||
        typeof data.swapBytes !== 'number'
    ) {
        fail('servidor');
    }
    return { cpuCores: data.cpuCores, memoryBytes: data.memoryBytes, swapBytes: data.swapBytes };
}

/** `shared::DeployKey`: parte pública de la llave de despliegue. */
export interface DeployKey {
    publicKey: string;
    fingerprint: string;
}

export function parseDeployKey(data: unknown): DeployKey | null {
    if (data === null) return null;
    if (!isRecord(data) || typeof data.publicKey !== 'string' || typeof data.fingerprint !== 'string') {
        fail('llave de despliegue');
    }
    return { publicKey: data.publicKey, fingerprint: data.fingerprint };
}

/** `shared::GitHubAccess`: cuenta con la que el servidor descarga repositorios privados. */
export interface GitHubAccess {
    login: string;
    /** Segundos Unix. */
    savedAt: number;
}

/** `shared::DeployRun`. */
export interface DeployRun {
    id: number;
    status: 'running' | 'succeeded' | 'failed';
    /** `deploy`: versión nueva; `rollback`: vuelta a la anterior. */
    kind: 'deploy' | 'rollback';
    /** `r<segundos Unix>-<nanosegundos>`. */
    release: string;
    branch: string;
    startedAt: number;
    finishedAt: number | null;
    detail: string | null;
}

/** Mismas reglas que `ServiceSlug` en el agente. */
export const SERVICE_NAME_PATTERN = /^[a-z0-9](?:[a-z0-9-]{0,46}[a-z0-9])?$/;

const isRecord = (value: unknown): value is Record<string, unknown> =>
    typeof value === 'object' && value !== null && !Array.isArray(value);

function fail(what: string): never {
    throw new TypeError(`respuesta del agente inesperada: ${what}`);
}

export function parseServiceSummary(data: unknown): ServiceSummary {
    if (!isRecord(data)) fail('servicio');
    const { name, displayName, description, unit, createdAt } = data;
    if (
        typeof name !== 'string' ||
        typeof displayName !== 'string' ||
        typeof description !== 'string' ||
        typeof unit !== 'string' ||
        typeof createdAt !== 'number'
    ) {
        fail('servicio');
    }
    return { name, displayName, description, unit, createdAt };
}

export function parseServiceList(data: unknown): ServiceSummary[] {
    if (!Array.isArray(data)) fail('lista de servicios');
    return data.map(parseServiceSummary);
}

const nullable = <T>(value: unknown, check: (value: unknown) => value is T): T | null =>
    value === null || value === undefined ? null : check(value) ? value : fail('entrada de log');
const isNumber = (value: unknown): value is number => typeof value === 'number';
const isString = (value: unknown): value is string => typeof value === 'string';

export function parseLogEvent(data: unknown): LogEvent {
    if (!isRecord(data)) fail('evento de log');
    if (data.type === 'entry') {
        const { timestampUs, message } = data;
        if (typeof timestampUs !== 'number' || typeof message !== 'string') fail('entrada de log');
        return {
            type: 'entry',
            timestampUs,
            message,
            priority: nullable(data.priority, isNumber),
            pid: nullable(data.pid, isNumber),
            identifier: nullable(data.identifier, isString),
        };
    }
    if (data.type === 'end') {
        const reason = LOG_END_REASONS.find((known) => known === data.reason);
        if (!reason) fail('fin de logs');
        return { type: 'end', reason, detail: nullable(data.detail, isString) };
    }
    return fail('evento de log');
}

/** Mismas reglas que la validación de claves del agente (`envfile::validate`). */
export const ENV_KEY_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/;

const isEnvVariable = (value: unknown): value is EnvVariable =>
    isRecord(value) && typeof value.key === 'string' && typeof value.value === 'string';

function parseVariables(data: unknown): EnvVariable[] {
    if (!Array.isArray(data) || !data.every(isEnvVariable)) fail('variables');
    return data.map(({ key, value }) => ({ key, value }));
}

export function parseServiceEnvironment(data: unknown): ServiceEnvironment {
    if (!isRecord(data)) fail('variables del servicio');
    const { name, unit, file, loadedByUnit, globalLoadedByUnit } = data;
    if (
        typeof name !== 'string' ||
        typeof unit !== 'string' ||
        typeof file !== 'string' ||
        typeof loadedByUnit !== 'boolean' ||
        typeof globalLoadedByUnit !== 'boolean'
    ) {
        fail('variables del servicio');
    }
    return { name, unit, file, loadedByUnit, globalLoadedByUnit, variables: parseVariables(data.variables) };
}

export function parseGlobalEnvironment(data: unknown): GlobalEnvironment {
    if (!isRecord(data) || typeof data.file !== 'string') fail('variables globales');
    return { file: data.file, variables: parseVariables(data.variables) };
}

export function parseServiceDeployment(data: unknown): ServiceDeployment {
    if (!isRecord(data)) fail('despliegue');
    const { name, repository, access, workingDirectory, setupCommands, startCommand, publishDir, updatedAt } = data;
    if (
        (access !== 'account' && access !== 'deployKey') ||
        !(publishDir === null || typeof publishDir === 'string') ||
        typeof name !== 'string' ||
        typeof workingDirectory !== 'string' ||
        typeof startCommand !== 'string' ||
        !Array.isArray(setupCommands) ||
        !setupCommands.every(isString) ||
        !(updatedAt === null || typeof updatedAt === 'number')
    ) {
        fail('despliegue');
    }
    let repo: RepositoryRef | null = null;
    if (repository !== null) {
        if (
            !isRecord(repository) ||
            typeof repository.owner !== 'string' ||
            typeof repository.name !== 'string' ||
            typeof repository.branch !== 'string'
        ) {
            fail('repositorio');
        }
        repo = { owner: repository.owner, name: repository.name, branch: repository.branch };
    }
    return { name, repository: repo, access, workingDirectory, setupCommands, startCommand, publishDir, updatedAt };
}

export function parseServiceStatus(data: unknown): ServiceStatus {
    if (!isRecord(data)) fail('estado');
    const { name, unit, loadState, activeState, subState, pid, metrics, deployed, release, previousRelease } = data;
    const load = LOAD_STATES.find((state) => state === loadState);
    const active = ACTIVE_STATES.find((state) => state === activeState);
    if (
        typeof name !== 'string' ||
        typeof unit !== 'string' ||
        typeof subState !== 'string' ||
        !load ||
        !active ||
        typeof deployed !== 'boolean' ||
        !(release === null || typeof release === 'string') ||
        !(previousRelease === null || typeof previousRelease === 'string') ||
        !(pid === null || typeof pid === 'number')
    ) {
        fail('estado');
    }
    let parsedMetrics: ServiceStatus['metrics'] = null;
    if (metrics !== null) {
        if (
            !isRecord(metrics) ||
            typeof metrics.rssBytes !== 'number' ||
            typeof metrics.rssMb !== 'number' ||
            typeof metrics.cpuPercent !== 'number'
        ) {
            fail('métricas');
        }
        parsedMetrics = { rssBytes: metrics.rssBytes, rssMb: metrics.rssMb, cpuPercent: metrics.cpuPercent };
    }
    return {
        name,
        unit,
        loadState: load,
        activeState: active,
        subState,
        pid,
        metrics: parsedMetrics,
        deployed,
        release,
        previousRelease,
        resources: parseResources(data.resources),
        host: parseHost(data.host),
    };
}

export function parseGitHubAccess(data: unknown): GitHubAccess | null {
    if (data === null) return null;
    if (!isRecord(data) || typeof data.login !== 'string' || typeof data.savedAt !== 'number') {
        fail('acceso a GitHub');
    }
    return { login: data.login, savedAt: data.savedAt };
}

const DEPLOY_STATUSES = ['running', 'succeeded', 'failed'] as const;

const DEPLOY_KINDS = ['deploy', 'rollback'] as const;

/** Fecha de construcción de una versión (sus segundos Unix van en el id), en milisegundos. */
export function releaseTime(release: string): number | null {
    const match = /^r(\d{10})-\d{9}$/.exec(release);
    return match?.[1] ? Number(match[1]) * 1000 : null;
}

export function parseDeployRun(data: unknown): DeployRun {
    if (!isRecord(data)) fail('despliegue');
    const status = DEPLOY_STATUSES.find((known) => known === data.status);
    const { id, branch, startedAt, finishedAt, detail, release } = data;
    const kind = DEPLOY_KINDS.find((known) => known === data.kind);
    if (
        !status ||
        !kind ||
        typeof release !== 'string' ||
        typeof id !== 'number' ||
        typeof branch !== 'string' ||
        typeof startedAt !== 'number' ||
        !(finishedAt === null || typeof finishedAt === 'number') ||
        !(detail === null || typeof detail === 'string')
    ) {
        fail('despliegue');
    }
    return { id, status, kind, release, branch, startedAt, finishedAt, detail };
}

export function parseDeployRuns(data: unknown): DeployRun[] {
    if (!Array.isArray(data)) fail('despliegues');
    return data.map(parseDeployRun);
}
