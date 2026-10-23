import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import { TunnelError } from '$lib/ssh';

import DeployKeyPanel from '../DeployKeyPanel.svelte';

const KEY = { publicKey: 'ssh-ed25519 AAAAC3Nza bbel-api@vps', fingerprint: 'SHA256:abc123' };
const REPO = { owner: 'acme', name: 'api', branch: 'main' };
const handlers = () => ({
    onCreate: vi.fn(async () => {}),
    onRegister: vi.fn(async () => {}),
    onOpenGitHub: vi.fn(),
});

describe('DeployKeyPanel', () => {
    test('sin llave: generarla', async () => {
        const props = handlers();
        await render(DeployKeyPanel, { key: null, repository: REPO, canRegister: false, ...props });
        await page.getByRole('button', { name: 'Generar llave' }).click();
        expect(props.onCreate).toHaveBeenCalledOnce();
    });

    test('con la App de BBEL: muestra la llave y el registro manual, sin pedir Administration', async () => {
        const props = handlers();
        await render(DeployKeyPanel, { key: KEY, repository: REPO, canRegister: false, ...props });
        await expect.element(page.getByText(KEY.publicKey)).toBeVisible();
        await expect.element(page.getByText('Huella SHA256:abc123')).toBeVisible();
        await expect.element(page.getByRole('button', { name: 'Registrar en GitHub' })).not.toBeInTheDocument();
        await expect.element(page.getByText(/tu propia GitHub App con permiso Administration/)).toBeVisible();
        await page.getByRole('button', { name: 'Abrir en GitHub' }).click();
        expect(props.onOpenGitHub).toHaveBeenCalledOnce();
    });

    test('con App propia: registro en un clic', async () => {
        const props = handlers();
        await render(DeployKeyPanel, { key: KEY, repository: REPO, canRegister: true, ...props });
        await page.getByRole('button', { name: 'Registrar en GitHub' }).click();
        expect(props.onRegister).toHaveBeenCalledOnce();
        await expect.element(page.getByRole('status')).toHaveTextContent('Registrada en acme/api.');
    });

    test('si la App no tiene el permiso, pasa al registro manual', async () => {
        const props = handlers();
        props.onRegister.mockRejectedValueOnce(new TunnelError('GITHUB_PERMISSION_MISSING', 'sin permiso'));
        await render(DeployKeyPanel, { key: KEY, repository: REPO, canRegister: true, ...props });
        await page.getByRole('button', { name: 'Registrar en GitHub' }).click();
        await expect.element(page.getByText(/no tiene el permiso Administration/)).toBeVisible();
        await page.getByRole('button', { name: 'Abrir en GitHub' }).click();
        expect(props.onOpenGitHub).toHaveBeenCalledOnce();
    });
});
