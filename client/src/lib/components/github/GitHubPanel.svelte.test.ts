import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import GitHubPanel from './GitHubPanel.svelte';

const SETTINGS = { clientId: 'Iv23liAbCdEf123456', appSlug: 'mi-blacksmith' };
const handlers = () => ({
    onConfigure: vi.fn(async () => {}),
    onUseDefault: vi.fn(async () => {}),
    onConnect: vi.fn(),
    onCancel: vi.fn(),
    onOpenVerification: vi.fn(),
    onInstall: vi.fn(),
    onDisconnect: vi.fn(async () => {}),
});

describe('GitHubPanel', () => {
    test('con la App del proyecto: conectar', async () => {
        const props = handlers();
        await render(GitHubPanel, { settings: SETTINGS, user: null, pending: null, ...props });
        await page.getByRole('button', { name: 'Conectar con GitHub' }).click();
        expect(props.onConnect).toHaveBeenCalledOnce();
    });

    test('App propia: pide Client ID y slug, y valida', async () => {
        const props = handlers();
        await render(GitHubPanel, { settings: SETTINGS, user: null, pending: null, ...props });
        await page.getByRole('button', { name: 'Usar mi propia GitHub App' }).click();
        await page.getByRole('button', { name: 'Guardar' }).click();
        await expect
            .element(page.getByRole('textbox', { name: /Client ID/ }))
            .toHaveAccessibleDescription(/Obligatorio/);
        await page.getByRole('textbox', { name: /Client ID/ }).fill(' Iv23liAbCdEf123456 ');
        await page.getByRole('textbox', { name: /Slug/ }).fill('Mal Slug');
        await page.getByRole('button', { name: 'Guardar' }).click();
        await expect
            .element(page.getByRole('textbox', { name: /Slug/ }))
            .toHaveAccessibleDescription(/Revisa el valor/);
        await page.getByRole('textbox', { name: /Slug/ }).fill('mi-blacksmith');
        await page.getByRole('button', { name: 'Guardar' }).click();
        expect(props.onConfigure).toHaveBeenCalledWith(SETTINGS);
    });

    test('con App propia: la indica y permite volver a la del proyecto', async () => {
        const props = handlers();
        await render(GitHubPanel, { settings: SETTINGS, custom: true, user: null, pending: null, ...props });
        await expect.element(page.getByText('Usando tu App «mi-blacksmith».')).toBeVisible();
        await page.getByRole('button', { name: 'Usar la App de BBEL' }).click();
        expect(props.onUseDefault).toHaveBeenCalledOnce();
    });

    test('esperando: muestra el código y permite cancelar o reabrir GitHub', async () => {
        const props = handlers();
        await render(GitHubPanel, {
            settings: SETTINGS,
            user: null,
            pending: { userCode: 'WDJB-MJHT', verificationUri: 'https://github.com/login/device', expiresIn: 900 },
            ...props,
        });
        await expect.element(page.getByText('WDJB-MJHT')).toBeVisible();
        await page.getByRole('button', { name: 'Abrir GitHub de nuevo' }).click();
        await page.getByRole('button', { name: 'Cancelar' }).click();
        expect(props.onOpenVerification).toHaveBeenCalledOnce();
        expect(props.onCancel).toHaveBeenCalledOnce();
    });

    test('conectado: dar acceso a más repositorios y desconectar tras confirmar', async () => {
        const props = handlers();
        await render(GitHubPanel, {
            settings: SETTINGS,
            user: { login: 'octocat', name: null, avatarUrl: null },
            pending: null,
            ...props,
        });
        await expect.element(page.getByText('octocat', { exact: true })).toBeVisible();
        await page.getByRole('button', { name: 'Dar acceso a más repositorios' }).click();
        expect(props.onInstall).toHaveBeenCalledOnce();
        await page.getByRole('button', { name: 'Desconectar' }).click();
        expect(props.onDisconnect).not.toHaveBeenCalled();
        await page.getByRole('alertdialog').getByRole('button', { name: 'Desconectar' }).click();
        await expect.poll(() => props.onDisconnect.mock.calls.length).toBe(1);
    });

    test('muestra el error del último intento', async () => {
        await render(GitHubPanel, {
            settings: SETTINGS,
            user: null,
            pending: null,
            error: 'Se rechazó la autorización en GitHub.',
            ...handlers(),
        });
        await expect.element(page.getByRole('alert')).toHaveTextContent('Se rechazó la autorización en GitHub.');
    });
});
