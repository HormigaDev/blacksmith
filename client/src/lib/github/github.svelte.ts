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

export type RepositoriesStatus = 'idle' | 'loading' | 'ready' | 'error';

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
    /** Repositorios accesibles; `null` hasta cargarlos (ver `repositoriesStatus`). */
    repositories = $state<GitHubRepository[] | null>(null);
    repositoriesStatus = $state<RepositoriesStatus>('idle');

    async load(): Promise<void> {
        try {
            const status = await gitHubStatus();
            this.settings = status.settings;
            this.custom = status.custom;
            this.user = status.user;
            if (this.user) void this.loadRepositories().catch(() => undefined);
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
        this.repositoriesStatus = 'idle';
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
            void this.loadRepositories(true).catch(() => undefined);
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

    async loadRepositories(force = false): Promise<GitHubRepository[]> {
        if (this.repositories && !force) return this.repositories;
        this.repositoriesStatus = 'loading';
        try {
            this.repositories = await gitHubRepositories();
            this.repositoriesStatus = 'ready';
        } catch (error) {
            this.repositoriesStatus = 'error';
            if (error instanceof TunnelError && error.code === 'GITHUB_NOT_CONNECTED') this.#signedOut();
            throw error;
        }
        return this.repositories;
    }
}

export const github = new GitHubState();
