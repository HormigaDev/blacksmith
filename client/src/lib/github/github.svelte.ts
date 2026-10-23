/**
 * Estado de GitHub en la aplicación: App configurada, cuenta conectada y el inicio de sesión en
 * curso (Device Flow). Única fuente de verdad para el botón de la cabecera y los formularios.
 */
import { openUrl } from '@tauri-apps/plugin-opener';

import { describeError } from '$lib/http';
import { TunnelError } from '$lib/ssh';

import {
    cancelGitHubLogin,
    configureGitHub,
    disconnectGitHub,
    type GitHubRepository,
    gitHubRepositories,
    type GitHubSettings,
    gitHubStatus,
    type GitHubUser,
    installUrl,
    type LoginStart,
    startGitHubLogin,
    useDefaultGitHubApp,
    waitGitHubLogin,
} from './api';

class GitHubState {
    settings = $state<GitHubSettings | null>(null);
    /** `true` si se usa una App propia en lugar de la del proyecto. */
    custom = $state(false);
    user = $state<GitHubUser | null>(null);
    loaded = $state(false);
    /** Código pendiente de confirmar en GitHub. */
    pending = $state<LoginStart | null>(null);
    busy = $state(false);
    error = $state<string | null>(null);
    /** Panel de conexión abierto (lo abre la cabecera o cualquier pantalla que necesite GitHub). */
    panelOpen = $state(false);
    /** Repositorios accesibles; `null` hasta cargarlos. */
    repositories = $state<GitHubRepository[] | null>(null);
    /** Por qué falló la última carga de repositorios. */
    repositoriesError = $state<string | null>(null);

    async load(): Promise<void> {
        try {
            const status = await gitHubStatus();
            this.settings = status.settings;
            this.custom = status.custom;
            this.user = status.user;
            if (this.user) void this.loadRepositories();
        } catch (error) {
            this.error = describeError(error);
        } finally {
            this.loaded = true;
        }
    }

    async configure(settings: GitHubSettings): Promise<void> {
        this.error = null;
        const saved = await configureGitHub(settings);
        if (this.settings?.clientId !== saved.clientId) this.#signedOut();
        this.settings = saved;
        this.custom = true;
    }

    /** Vuelve a la App del proyecto. */
    async useDefault(): Promise<void> {
        this.error = null;
        const saved = await useDefaultGitHubApp();
        if (this.custom) this.#signedOut();
        this.settings = saved;
        this.custom = false;
    }

    #signedOut(): void {
        this.user = null;
        this.repositories = null;
        this.repositoriesError = null;
    }

    /** Pide el código, abre GitHub en el navegador y espera la confirmación. */
    async connect(): Promise<void> {
        this.error = null;
        this.busy = true;
        try {
            const start = await startGitHubLogin();
            this.pending = start;
            void openUrl(start.verificationUri).catch(() => undefined);
            this.user = await waitGitHubLogin();
            this.repositories = null;
            void this.loadRepositories(true);
        } catch (error) {
            if (!(error instanceof TunnelError && error.code === 'GITHUB_LOGIN_CANCELLED')) {
                this.error = describeError(error);
            }
        } finally {
            this.pending = null;
            this.busy = false;
        }
    }

    async cancel(): Promise<void> {
        await cancelGitHubLogin();
    }

    async disconnect(): Promise<void> {
        await disconnectGitHub();
        this.#signedOut();
    }

    /** Página de GitHub para dar acceso a más repositorios. */
    async openInstall(): Promise<void> {
        if (this.settings) await openUrl(installUrl(this.settings.appSlug));
    }

    async loadRepositories(force = false): Promise<void> {
        if (this.repositories && !force) return;
        const user = this.user;
        this.repositoriesError = null;
        try {
            const repositories = await gitHubRepositories();
            // Si entretanto se desconectó (o cambió) la cuenta, la respuesta ya no es de nadie.
            if (this.user === user) this.repositories = repositories;
        } catch (error) {
            if (this.user !== user) return;
            if (error instanceof TunnelError && error.code === 'GITHUB_NOT_CONNECTED') this.#signedOut();
            else this.repositoriesError = describeError(error);
        }
    }
}

export const github = new GitHubState();
