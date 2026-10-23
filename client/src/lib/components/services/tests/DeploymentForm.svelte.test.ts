import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { ServiceDeployment } from '$lib/agent';
import type { GitHubRepository } from '$lib/github';

import DeploymentForm from '../DeploymentForm.svelte';

const EMPTY: ServiceDeployment = {
    name: 'api',
    repository: null,
    access: 'account',
    workingDirectory: '/srv/bbel/api',
    setupCommands: [],
    startCommand: '',
    publishDir: null,
    updatedAt: null,
};

const REPO: GitHubRepository = {
    owner: 'acme',
    name: 'api',
    fullName: 'acme/api',
    private: true,
    defaultBranch: 'main',
    htmlUrl: 'https://github.com/acme/api',
};

describe('DeploymentForm', () => {
    test('sin GitHub pide conectarlo; el directorio de trabajo se muestra', async () => {
        const onConnectGitHub = vi.fn();
        await render(DeploymentForm, {
            deployment: EMPTY,
            githubConnected: false,
            repositories: null,
            loadBranches: vi.fn(),
            onConnectGitHub,
            onSave: vi.fn(),
        });
        await expect.element(page.getByText('/srv/bbel/api')).toBeVisible();
        await page.getByRole('button', { name: 'Conectar con GitHub' }).click();
        expect(onConnectGitHub).toHaveBeenCalledOnce();
    });

    test('elige repositorio (rama por defecto), comandos en orden y arranque', async () => {
        const onSave = vi.fn(async () => {});
        const loadBranches = vi.fn(async () => ['develop', 'main']);
        await render(DeploymentForm, {
            deployment: EMPTY,
            githubConnected: true,
            repositories: [REPO],
            loadBranches,
            onSave,
        });

        await page.getByRole('button', { name: 'Repositorio' }).click();
        await page.getByRole('option', { name: 'acme/api' }).click();
        expect(loadBranches).toHaveBeenCalledWith('acme', 'api');
        await expect.element(page.getByRole('button', { name: 'Rama' })).toHaveTextContent('main');

        await page.getByRole('button', { name: 'Añadir comando' }).click();
        await page.getByRole('textbox', { name: 'Comando 1' }).fill('npm run build');
        await page.getByRole('button', { name: 'Añadir comando' }).click();
        await page.getByRole('textbox', { name: 'Comando 2' }).fill('npm ci');
        await page.getByRole('button', { name: 'Subir comando 2' }).click();
        await page.getByRole('textbox', { name: /Comando de arranque/ }).fill(' node dist/server.js ');
        await page.getByRole('button', { name: 'Guardar' }).click();

        expect(onSave).toHaveBeenCalledWith({
            repository: { owner: 'acme', name: 'api', branch: 'main' },
            access: 'account',
            setupCommands: ['npm ci', 'npm run build'],
            startCommand: 'node dist/server.js',
            publishDir: null,
        });
    });

    test('un comando vacío no se envía', async () => {
        const onSave = vi.fn(async () => {});
        await render(DeploymentForm, {
            deployment: EMPTY,
            githubConnected: true,
            repositories: [],
            loadBranches: vi.fn(),
            onSave,
        });
        await page.getByRole('button', { name: 'Añadir comando' }).click();
        await page.getByRole('button', { name: 'Guardar' }).click();
        await expect
            .element(page.getByRole('textbox', { name: 'Comando 1' }))
            .toHaveAccessibleDescription(/El comando está vacío/);
        expect(onSave).not.toHaveBeenCalled();
    });

    test('el repositorio guardado queda fijo; la rama se puede cambiar', async () => {
        const onSave = vi.fn(async () => {});
        const loadBranches = vi.fn(async () => ['main', 'develop']);
        await render(DeploymentForm, {
            deployment: { ...EMPTY, repository: { owner: 'old', name: 'repo', branch: 'main' }, startCommand: 'x' },
            githubConnected: true,
            repositories: [REPO],
            loadBranches,
            onSave,
        });
        await expect.element(page.getByText('old/repo')).toBeVisible();
        await expect.element(page.getByRole('button', { name: 'Repositorio' })).not.toBeInTheDocument();
        expect(loadBranches).toHaveBeenCalledWith('old', 'repo');
        await page.getByRole('button', { name: 'Rama' }).click();
        await page.getByRole('option', { name: 'develop' }).click();
        await page.getByRole('button', { name: 'Guardar' }).click();
        expect(onSave).toHaveBeenCalledWith({
            repository: { owner: 'old', name: 'repo', branch: 'develop' },
            access: 'account',
            setupCommands: [],
            startCommand: 'x',
            publishDir: null,
        });
    });

    test('sin GitHub, la rama de un repositorio fijado se escribe a mano', async () => {
        await render(DeploymentForm, {
            deployment: { ...EMPTY, repository: { owner: 'acme', name: 'api', branch: 'main' } },
            githubConnected: false,
            repositories: null,
            loadBranches: vi.fn(),
            onSave: vi.fn(),
        });
        await expect.element(page.getByRole('textbox', { name: 'Rama' })).toHaveValue('main');
    });

    test('acceso con llave de despliegue: se guarda con la definición', async () => {
        const onSave = vi.fn(async () => {});
        await render(DeploymentForm, {
            deployment: { ...EMPTY, repository: { owner: 'acme', name: 'api', branch: 'main' }, startCommand: 'x' },
            githubConnected: false,
            repositories: null,
            loadBranches: vi.fn(),
            onSave,
        });
        await expect.element(page.getByText(/con la cuenta de GitHub conectada/)).toBeVisible();
        await page.getByRole('radio', { name: 'Llave de despliegue' }).click();
        await expect.element(page.getByText(/una llave propia, de solo lectura/)).toBeVisible();
        await page.getByRole('button', { name: 'Guardar' }).click();
        expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ access: 'deployKey' }));
    });

    test('carpeta pública: relativa al repositorio, sin salir de él', async () => {
        const onSave = vi.fn(async () => {});
        await render(DeploymentForm, {
            deployment: { ...EMPTY, repository: { owner: 'acme', name: 'web', branch: 'main' } },
            githubConnected: false,
            repositories: null,
            loadBranches: vi.fn(),
            onSave,
        });
        const publish = page.getByRole('textbox', { name: /Carpeta pública/ });
        await publish.fill('../etc');
        await page.getByRole('button', { name: 'Guardar' }).click();
        await expect.element(publish).toHaveAccessibleDescription(/Una ruta dentro del repositorio/);
        expect(onSave).not.toHaveBeenCalled();
        await publish.fill(' dist/ ');
        await page.getByRole('button', { name: 'Guardar' }).click();
        expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ publishDir: 'dist', startCommand: '' }));
    });
});
