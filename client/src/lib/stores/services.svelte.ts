/**
 * Estado de los servicios de cada servidor, compartido por toda la interfaz: una sola copia por
 * servidor y por servicio, así cualquier pantalla ve al momento lo que otra cambió.
 *
 * - `serviceStore.server(id)`: la lista de servicios del servidor, sus variables globales, la
 *   cuenta de GitHub con la que descarga repositorios privados y los datos del servidor
 *   (sistema, hardware y uso).
 * - `serviceStore.service(id, nombre)`: estado, despliegue, variables, llave de despliegue (si
 *   el servicio la usa), sitio de nginx, límites de recursos y despliegues.
 *
 * Las cargas guardan su error en el propio objeto (para mostrarlo donde corresponda); las acciones
 * lanzan, para que el formulario que las pidió muestre el motivo.
 */
import {
    agentClient,
    createDeployKey,
    deploy,
    type DeployKey,
    deployKey,
    type DeployRun,
    rollback,
    deployRuns,
    type EnvVariable,
    type GitHubAccess,
    githubAccess,
    type GlobalEnvironment,
    globalEnvironment,
    type HostInfo,
    hostInfo,
    listServices,
    type RegisterService,
    registerService,
    removeServiceSite,
    type ResourceLimits,
    retryCertificate,
    saveServiceSite,
    saveGlobalEnvironment,
    saveServiceDeployment,
    saveServiceEnvironment,
    saveServiceLimits,
    type ServiceAction,
    serviceAction,
    type ServiceDeployment,
    serviceDeployment,
    type ServiceEnvironment,
    serviceEnvironment,
    type ServiceLimits,
    serviceLimits,
    type ServiceStatus,
    serviceSite,
    serviceStatus,
    type Site,
    type SiteConfig,
    type ServiceSummary,
    shareGitHubWithServer,
    unregisterService,
    type UpdateDeployment,
} from '$lib/agent';
import { describeError, type HttpClient } from '$lib/http';
import { appendSample, type ResourceSample } from '$lib/services/resource-history';

/** Cada cuánto se refresca el estado de un servicio a la vista. */
export const STATUS_INTERVAL_MS = 3000;

/** Más a menudo mientras hay un despliegue en curso. */
export const DEPLOYING_INTERVAL_MS = 2000;

export class ServerServices {
    readonly serverId: string;
    /** `null` hasta la primera carga. */
    list = $state<ServiceSummary[] | null>(null);
    error = $state<string | null>(null);
    global = $state<GlobalEnvironment | null>(null);
    /** Cuenta de GitHub del servidor: `undefined` sin cargar; `null` si no tiene. */
    github = $state<GitHubAccess | null | undefined>(undefined);
    /** Sistema, hardware y uso del servidor: `null` hasta la primera carga. */
    host = $state<HostInfo | null>(null);
    hostError = $state<string | null>(null);

    constructor(serverId: string) {
        this.serverId = serverId;
    }

    get client(): HttpClient {
        return agentClient(this.serverId);
    }

    find(name: string): ServiceSummary | undefined {
        return this.list?.find((service) => service.name === name);
    }

    async load(): Promise<void> {
        try {
            this.list = await listServices(this.client);
            this.error = null;
        } catch (error) {
            this.error = describeError(error);
        }
    }

    async register(service: RegisterService): Promise<ServiceSummary> {
        const created = await registerService(this.client, service);
        this.list = [...(this.list ?? []), created];
        return created;
    }

    async remove(name: string): Promise<void> {
        await unregisterService(this.client, name);
        this.list = (this.list ?? []).filter((service) => service.name !== name);
        serviceStore.drop(this.serverId, name);
    }

    /** Datos del servidor. Un fallo puntual conserva los últimos conocidos. */
    async loadHost(): Promise<void> {
        try {
            this.host = await hostInfo(this.client);
            this.hostError = null;
        } catch (error) {
            this.hostError = describeError(error);
        }
    }

    async loadGlobal(): Promise<void> {
        this.global = await globalEnvironment(this.client);
    }

    async saveGlobal(variables: EnvVariable[]): Promise<void> {
        this.global = await saveGlobalEnvironment(this.client, variables);
    }

    async loadGitHub(): Promise<void> {
        this.github = await githubAccess(this.client);
    }

    /**
     * Deja en el servidor la cuenta de GitHub conectada en este equipo (`login`), para que
     * despliegue repositorios privados por su cuenta, también con este equipo apagado. Sin
     * cuenta conectada no hace nada: los repositorios públicos no la necesitan.
     */
    async shareGitHub(login: string | null): Promise<void> {
        if (!login) return;
        this.github = await shareGitHubWithServer(this.serverId);
    }
}

export class ServiceEntry {
    readonly serverId: string;
    readonly name: string;
    status = $state<ServiceStatus | null>(null);
    /** Acción de ciclo de vida en curso. */
    busy = $state<ServiceAction | null>(null);
    deployment = $state<ServiceDeployment | null>(null);
    env = $state<ServiceEnvironment | null>(null);
    /** Llave de despliegue: `undefined` sin cargar; `null` si aún no se generó. */
    key = $state<DeployKey | null | undefined>(undefined);
    runs = $state<DeployRun[] | null>(null);
    /** Últimas lecturas de consumo (la más reciente al final). Solo mientras alguien mira. */
    history = $state.raw<readonly ResourceSample[]>([]);
    /** Sitio de nginx: `undefined` sin cargar; `null` si no tiene. */
    site = $state<Site | null | undefined>(undefined);
    /** Límites de recursos: `null` hasta cargarlos. */
    limits = $state<ServiceLimits | null>(null);
    /** Error de la última carga (despliegue o variables). */
    error = $state<string | null>(null);

    #watchers = 0;
    #timer: ReturnType<typeof setTimeout> | undefined;

    constructor(serverId: string, name: string) {
        this.serverId = serverId;
        this.name = name;
    }

    get client(): HttpClient {
        return agentClient(this.serverId);
    }

    get deploying(): boolean {
        return this.runs?.[0]?.status === 'running';
    }

    /** Se está pidiendo el certificado del sitio. */
    get issuing(): boolean {
        return this.site?.tls === 'issuing';
    }

    /** Refresca estado y despliegues. Un fallo puntual conserva lo último conocido. */
    async refresh(): Promise<void> {
        const [status, runs] = await Promise.allSettled([
            serviceStatus(this.client, this.name),
            deployRuns(this.client, this.name),
        ]);
        if (status.status === 'fulfilled') {
            this.status = status.value;
            this.#record(status.value);
        }
        if (runs.status === 'fulfilled') this.runs = runs.value;
        // Mientras se pide el certificado, también el sitio (pasa a HTTPS o falla).
        if (this.issuing) await this.loadSite().catch(() => undefined);
    }

    #record(status: ServiceStatus): void {
        this.history = appendSample(this.history, status);
    }

    /** Mantiene el estado al día mientras haya alguien mirando. Devuelve la función para dejar
     * de mirar. */
    watch(): () => void {
        this.#watchers++;
        if (this.#watchers === 1) void this.#tick();
        return () => {
            this.#watchers--;
            if (this.#watchers === 0 && this.#timer) {
                clearTimeout(this.#timer);
                this.#timer = undefined;
            }
        };
    }

    async #tick(): Promise<void> {
        await this.refresh();
        if (this.#watchers === 0) return;
        const delay = this.deploying || this.issuing ? DEPLOYING_INTERVAL_MS : STATUS_INTERVAL_MS;
        this.#timer = setTimeout(() => void this.#tick(), delay);
    }

    async act(action: ServiceAction): Promise<void> {
        this.busy = action;
        try {
            await serviceAction(this.client, this.name, action);
        } finally {
            this.busy = null;
            await this.refresh();
        }
    }

    async load(): Promise<void> {
        try {
            const [deployment, env] = await Promise.all([
                serviceDeployment(this.client, this.name),
                serviceEnvironment(this.client, this.name),
            ]);
            this.deployment = deployment;
            this.env = env;
            this.error = null;
        } catch (error) {
            this.error = describeError(error);
        }
    }

    /** Guarda la definición. Con `githubLogin`, el servidor recibe también la cuenta de GitHub. */
    async saveDeployment(update: UpdateDeployment, githubLogin: string | null = null): Promise<void> {
        this.deployment = await saveServiceDeployment(this.client, this.name, update);
        await serviceStore.server(this.serverId).shareGitHub(githubLogin);
    }

    async loadSite(): Promise<void> {
        this.site = await serviceSite(this.client, this.name);
    }

    /** Guarda el sitio y lo aplica en nginx. Con HTTPS, el certificado llega en segundo plano. */
    async saveSite(config: SiteConfig): Promise<Site> {
        this.site = await saveServiceSite(this.client, this.name, config);
        return this.site;
    }

    async removeSite(): Promise<void> {
        await removeServiceSite(this.client, this.name);
        this.site = null;
    }

    async retryCertificate(): Promise<void> {
        this.site = await retryCertificate(this.client, this.name);
    }

    async loadLimits(): Promise<void> {
        this.limits = await serviceLimits(this.client, this.name);
    }

    /** Los aplica al momento; el estado refleja los nuevos límites en la siguiente lectura. */
    async saveLimits(limits: ResourceLimits): Promise<void> {
        this.limits = await saveServiceLimits(this.client, this.name, limits);
        await this.refresh();
    }

    async loadKey(): Promise<void> {
        this.key = await deployKey(this.client, this.name);
    }

    async createKey(): Promise<DeployKey> {
        this.key = await createDeployKey(this.client, this.name);
        return this.key;
    }

    async saveEnv(variables: EnvVariable[]): Promise<void> {
        this.env = await saveServiceEnvironment(this.client, this.name, variables);
    }

    /**
     * Lanza un despliegue. Con `githubLogin`, antes renueva la cuenta de GitHub del servidor: el
     * servidor clona y prepara solo, sin que este equipo intervenga después.
     */
    async deploy(githubLogin: string | null = null): Promise<DeployRun> {
        await serviceStore.server(this.serverId).shareGitHub(githubLogin);
        return this.#started(await deploy(this.client, this.name));
    }

    /** Vuelve a la versión anterior (ya construida en el servidor). */
    async rollback(): Promise<DeployRun> {
        return this.#started(await rollback(this.client, this.name));
    }

    #started(run: DeployRun): DeployRun {
        this.runs = [run, ...(this.runs ?? [])];
        // Reprograma el sondeo al ritmo de despliegue.
        if (this.#watchers > 0) {
            if (this.#timer) clearTimeout(this.#timer);
            this.#timer = setTimeout(() => void this.#tick(), DEPLOYING_INTERVAL_MS);
        }
        return run;
    }
}

class ServiceStore {
    #servers = new Map<string, ServerServices>();
    #services = new Map<string, ServiceEntry>();

    server(serverId: string): ServerServices {
        let server = this.#servers.get(serverId);
        if (!server) {
            server = new ServerServices(serverId);
            this.#servers.set(serverId, server);
        }
        return server;
    }

    service(serverId: string, name: string): ServiceEntry {
        const key = `${serverId}\u0000${name}`;
        let service = this.#services.get(key);
        if (!service) {
            service = new ServiceEntry(serverId, name);
            this.#services.set(key, service);
        }
        return service;
    }

    drop(serverId: string, name: string): void {
        this.#services.delete(`${serverId}\u0000${name}`);
    }

    /** Olvida todo lo de un servidor (p. ej. al quitarlo). */
    forget(serverId: string): void {
        this.#servers.delete(serverId);
        for (const key of [...this.#services.keys()]) {
            if (key.startsWith(`${serverId}\u0000`)) this.#services.delete(key);
        }
    }
}

export const serviceStore = new ServiceStore();
