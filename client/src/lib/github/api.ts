/**
 * Contrato con los comandos de GitHub en Rust (client/src-tauri/src/github). El token nunca llega
 * a la webview: Rust lo guarda cifrado y hace todas las peticiones.
 */
import { invoke, isTauri } from '@tauri-apps/api/core';

import { toTunnelError } from '$lib/ssh';

export interface GitHubSettings {
    clientId: string;
    appSlug: string;
}

export interface GitHubUser {
    login: string;
    name: string | null;
    avatarUrl: string | null;
}

export interface GitHubStatus {
    settings: GitHubSettings | null;
    /** `true` si se usa una App propia en lugar de la del proyecto. */
    custom: boolean;
    user: GitHubUser | null;
}

export interface LoginStart {
    userCode: string;
    verificationUri: string;
    /** Segundos. */
    expiresIn: number;
}

export interface GitHubRepository {
    owner: string;
    name: string;
    fullName: string;
    private: boolean;
    defaultBranch: string;
    htmlUrl: string;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    try {
        return await invoke<T>(command, args);
    } catch (error) {
        throw toTunnelError(error);
    }
}

/** Fuera de Tauri (navegador, tests sin simulación) no hay GitHub. */
export async function gitHubStatus(): Promise<GitHubStatus> {
    if (!isTauri()) return { settings: null, custom: false, user: null };
    return call('github_status');
}

export const configureGitHub = (settings: GitHubSettings): Promise<GitHubSettings> =>
    call('github_configure', { clientId: settings.clientId, appSlug: settings.appSlug });
/** Vuelve a la App del proyecto (desconecta la cuenta si se usaba otra). */
export const useDefaultGitHubApp = (): Promise<GitHubSettings> => call('github_use_default_app');
/**
 * Registra la llave de despliegue en el repositorio (solo lectura). Solo con una App propia con
 * permiso *Administration*; con la de BBEL falla con `GITHUB_PERMISSION_MISSING`.
 */
export const addDeployKey = (owner: string, name: string, title: string, key: string): Promise<void> =>
    call('github_add_deploy_key', { owner, name, title, key });
export const startGitHubLogin = (): Promise<LoginStart> => call('github_login_start');
/** Espera a que se confirme el código; se resuelve con el usuario conectado. */
export const waitGitHubLogin = (): Promise<GitHubUser> => call('github_login_wait');
export const cancelGitHubLogin = (): Promise<boolean> => call('github_login_cancel');
export const disconnectGitHub = (): Promise<boolean> => call('github_logout');
export const gitHubRepositories = (): Promise<GitHubRepository[]> => call('github_repositories');
export const gitHubBranches = (owner: string, name: string): Promise<string[]> =>
    call('github_branches', { owner, name });

/** Página para instalar la App en más cuentas o repositorios. */
export function installUrl(appSlug: string): string {
    return `https://github.com/apps/${encodeURIComponent(appSlug)}/installations/new`;
}
