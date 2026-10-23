import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { Site } from '$lib/agent';

import SiteForm from '../SiteForm.svelte';

const SITE: Site = {
    name: 'web',
    template: 'app',
    domains: ['app.example.com', 'www.app.example.com'],
    port: 3000,
    https: true,
    email: null,
    tls: 'active',
    tlsDetail: null,
    configPath: '/var/lib/blacksmith/nginx/sites/web.conf',
    updatedAt: 1,
};
const handlers = () => ({
    onSave: vi.fn(async () => {}),
    onRemove: vi.fn(async () => {}),
    onRetryCertificate: vi.fn(async () => {}),
});

describe('SiteForm', () => {
    test('nuevo: aplicación web con dominios, puerto y HTTPS', async () => {
        const props = handlers();
        await render(SiteForm, { site: null, publishDir: null, ...props });
        await page.getByRole('textbox', { name: 'Dominio' }).fill(' App.Example.com. ');
        await page.getByRole('textbox', { name: /Otros dominios/ }).fill('www.app.example.com');
        await page.getByRole('spinbutton', { name: /Puerto del servicio/ }).fill('3000');
        await page.getByRole('textbox', { name: /Correo/ }).fill('ops@example.com');
        await page.getByRole('button', { name: 'Guardar y aplicar' }).click();
        expect(props.onSave).toHaveBeenCalledWith({
            template: 'app',
            domains: ['app.example.com', 'www.app.example.com'],
            port: 3000,
            https: true,
            email: 'ops@example.com',
        });
    });

    test('valida como el agente: dominio, puerto y HTTPS en dominios locales', async () => {
        const props = handlers();
        await render(SiteForm, { site: null, publishDir: null, ...props });
        await page.getByRole('textbox', { name: 'Dominio' }).fill('app.localhost');
        await page.getByRole('spinbutton', { name: /Puerto del servicio/ }).fill('4040');
        await page.getByRole('button', { name: 'Guardar y aplicar' }).click();
        await expect
            .element(page.getByRole('spinbutton', { name: /Puerto del servicio/ }))
            .toHaveAccessibleDescription(/puerto es el del agente/);
        await expect.element(page.getByText(/solo emite certificados para dominios públicos/)).toBeVisible();
        expect(props.onSave).not.toHaveBeenCalled();

        await page.getByRole('switch', { name: /HTTPS/ }).click();
        await page.getByRole('spinbutton', { name: /Puerto del servicio/ }).fill('5173');
        await page.getByRole('button', { name: 'Guardar y aplicar' }).click();
        expect(props.onSave).toHaveBeenCalledWith(
            expect.objectContaining({ domains: ['app.localhost'], port: 5173, https: false, email: null }),
        );
    });

    test('una plantilla de archivos pide la carpeta pública y no guarda sin ella', async () => {
        const props = handlers();
        await render(SiteForm, { site: null, publishDir: null, ...props });
        await page.getByRole('button', { name: 'Tipo de proyecto' }).click();
        await page.getByRole('option', { name: 'Sitio estático' }).click();
        await expect.element(page.getByText(/define la carpeta pública/)).toBeVisible();
        await expect.element(page.getByRole('spinbutton', { name: /Puerto/ })).not.toBeInTheDocument();
        await page.getByRole('textbox', { name: 'Dominio' }).fill('example.com');
        await page.getByRole('button', { name: 'Guardar y aplicar' }).click();
        expect(props.onSave).not.toHaveBeenCalled();
    });

    test('certificado fallido: muestra el motivo y permite reintentar', async () => {
        const props = handlers();
        await render(SiteForm, {
            site: { ...SITE, tls: 'failed', tlsDetail: 'Timeout during connect (likely firewall problem)' },
            publishDir: null,
            ...props,
        });
        await expect.element(page.getByText('Certificado fallido')).toBeVisible();
        await expect.element(page.getByText(/likely firewall problem/)).toBeVisible();
        await page.getByRole('button', { name: 'Reintentar certificado' }).click();
        expect(props.onRetryCertificate).toHaveBeenCalledOnce();
    });

    test('con HTTPS activo abre la dirección https; quitar el sitio se confirma', async () => {
        const props = handlers();
        const onOpenSite = vi.fn();
        await render(SiteForm, { site: SITE, publishDir: null, onOpenSite, ...props });
        await expect.element(page.getByText('HTTPS activo')).toBeVisible();
        await page.getByRole('button', { name: 'Abrir en el navegador' }).click();
        expect(onOpenSite).toHaveBeenCalledWith('https://app.example.com');
        await page.getByRole('button', { name: 'Quitar sitio' }).click();
        expect(props.onRemove).not.toHaveBeenCalled();
        await page.getByRole('alertdialog').getByRole('button', { name: 'Quitar sitio' }).click();
        await expect.poll(() => props.onRemove.mock.calls.length).toBe(1);
    });
});
