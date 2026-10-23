import { createRawSnippet } from 'svelte';
import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { ServerEntry } from '$lib/servers/servers.svelte';

import ServerCard from '../ServerCard.svelte';

const PROFILE = {
    id: 'deploy@vps.example.com:22',
    kind: 'ssh' as const,
    host: 'vps.example.com',
    port: 22,
    username: 'deploy',
};
const LOCAL = { id: 'local:4040', kind: 'local' as const, host: 'localhost', port: 4040, username: '' };
const AGENT = { name: 'blacksmith-agent', version: '1.2.3', status: 'ready' as const };

function setup(entry: Partial<ServerEntry> = {}) {
    const handlers = {
        onConnect: vi.fn(),
        onDisconnect: vi.fn(),
        onRemove: vi.fn(),
        onForgetHostKey: vi.fn(),
    };
    const props = { entry: { profile: PROFILE, status: 'disconnected' as const, ...entry }, ...handlers };
    return { render: () => render(ServerCard, props), ...handlers };
}

describe('ServerCard', () => {
    test('desconectado: muestra el destino y conecta', async () => {
        const card = setup();
        await card.render();
        await expect.element(page.getByRole('heading', { name: 'deploy@vps.example.com' })).toBeVisible();
        await expect.element(page.getByText('Desconectado')).toBeVisible();
        await page.getByRole('button', { name: 'Conectar' }).click();
        expect(card.onConnect).toHaveBeenCalledOnce();
    });

    test('agente no configurado: lo explica y ofrece reintentar', async () => {
        const card = setup({ status: 'agentMissing' });
        await card.render();
        await expect.element(page.getByText('Agente no configurado')).toBeVisible();
        await expect.element(page.getByText(/el agente de Blacksmith no responde/)).toBeVisible();
        await page.getByRole('button', { name: 'Reintentar' }).click();
        expect(card.onConnect).toHaveBeenCalledOnce();
    });

    test('conectado: muestra el puerto local y desconecta', async () => {
        const card = setup({
            status: 'connected',
            tunnel: { targetId: PROFILE.id, localPort: 51234, remotePort: 4040, agent: AGENT },
        });
        await card.render();
        await expect.element(page.getByText('Túnel activo en 127.0.0.1:51.234')).toBeVisible();
        await expect.element(page.getByText('Agente 1.2.3')).toBeVisible();
        await page.getByRole('button', { name: 'Desconectar' }).click();
        expect(card.onDisconnect).toHaveBeenCalledOnce();
    });

    test('quitar el servidor exige confirmación', async () => {
        const card = setup({ profile: { ...PROFILE, port: 2222, id: 'deploy@vps.example.com:2222' } });
        await card.render();
        await page.getByRole('button', { name: 'Quitar deploy@vps.example.com:2222' }).click();
        expect(card.onRemove).not.toHaveBeenCalled();
        await page.getByRole('button', { name: 'Quitar', exact: true }).click();
        await expect.poll(() => card.onRemove.mock.calls.length).toBe(1);
    });

    test('llave de servidor cambiada: ofrece olvidarla, solo tras confirmar', async () => {
        const card = setup({ status: 'error', errorCode: 'HOST_KEY_CHANGED', error: 'La llave del servidor cambió.' });
        await card.render();
        await expect.element(page.getByText('La llave del servidor cambió.')).toBeVisible();
        await page.getByRole('button', { name: 'Olvidar la llave registrada' }).click();
        expect(card.onForgetHostKey).not.toHaveBeenCalled();
        await page.getByRole('button', { name: 'Olvidar llave' }).click();
        await expect.poll(() => card.onForgetHostKey.mock.calls.length).toBe(1);
    });

    test('local: dirección, insignia y ayuda propias, sin túnel', async () => {
        const card = setup({ profile: LOCAL, status: 'agentMissing' });
        await card.render();
        await expect.element(page.getByRole('heading', { name: 'localhost:4040' })).toBeVisible();
        await expect.element(page.getByText('Este equipo')).toBeVisible();
        await expect.element(page.getByText(/No hay ningún agente escuchando en 127\.0\.0\.1:4040/)).toBeVisible();
    });

    test('local conectado: conexión directa', async () => {
        const card = setup({
            profile: LOCAL,
            status: 'connected',
            tunnel: { targetId: LOCAL.id, localPort: 4040, remotePort: 4040, agent: AGENT },
        });
        await card.render();
        await expect.element(page.getByText('Conexión directa a 127.0.0.1:4040, sin túnel')).toBeVisible();
    });

    test('tarjeta de cuadrícula con asa: muestra el asa y las mismas acciones', async () => {
        const card = setup();
        await render(ServerCard, {
            entry: { profile: PROFILE, status: 'disconnected' },
            onConnect: card.onConnect,
            onDisconnect: card.onDisconnect,
            onRemove: card.onRemove,
            onForgetHostKey: card.onForgetHostKey,
            layout: 'tile',
            handle: createRawSnippet(() => ({ render: () => '<button type="button">Asa</button>' })),
        });
        await expect.element(page.getByRole('button', { name: 'Asa' })).toBeVisible();
        await page.getByRole('button', { name: 'Conectar' }).click();
        expect(card.onConnect).toHaveBeenCalledOnce();
    });

    test('con href, la tarjeta entera lleva a la página del servidor y los botones siguen siendo botones', async () => {
        const card = setup();
        await render(ServerCard, {
            entry: { profile: PROFILE, status: 'disconnected' },
            onConnect: card.onConnect,
            onDisconnect: card.onDisconnect,
            onRemove: card.onRemove,
            onForgetHostKey: card.onForgetHostKey,
            href: '/servers/deploy%40vps.example.com%3A22',
        });
        const link = page.getByRole('link', { name: 'deploy@vps.example.com' });
        await expect.element(link).toHaveAttribute('href', '/servers/deploy%40vps.example.com%3A22');
        // El área del enlace cubre la tarjeta, pero los botones quedan por encima.
        await page.getByRole('button', { name: 'Conectar' }).click();
        expect(card.onConnect).toHaveBeenCalledOnce();
        expect(location.pathname).not.toContain('/servers/');
    });

    test('sin href no hay enlace', async () => {
        const card = setup();
        await card.render();
        await expect.element(page.getByRole('link')).not.toBeInTheDocument();
    });
});
