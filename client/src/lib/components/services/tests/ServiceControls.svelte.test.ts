import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { ServiceStatus } from '$lib/agent';

import ServiceControls from '../ServiceControls.svelte';

const status = (overrides: Partial<ServiceStatus>): ServiceStatus => ({
    name: 'api',
    unit: 'bbel-api.service',
    loadState: 'loaded',
    activeState: 'inactive',
    subState: 'dead',
    pid: null,
    metrics: null,
    deployed: true,
    release: 'r1800000000-000000000',
    previousRelease: null,
    ...overrides,
});

const button = (name: string) => page.getByRole('button', { name });

describe('ServiceControls', () => {
    test('sin desplegar: todo deshabilitado y explicado', async () => {
        await render(ServiceControls, {
            name: 'API',
            status: status({ deployed: false }),
            onAction: vi.fn(),
        });
        await expect.element(button('Iniciar')).toBeDisabled();
        await expect.element(button('Reiniciar')).toBeDisabled();
        await expect.element(page.getByText('Sin desplegar')).toBeVisible();
    });

    test('detenido: iniciar sin confirmar; reiniciar deshabilitado', async () => {
        const onAction = vi.fn();
        await render(ServiceControls, { name: 'API', status: status({}), onAction });
        await expect.element(page.getByText('Detenido')).toBeVisible();
        await expect.element(button('Reiniciar')).toBeDisabled();
        await button('Iniciar').click();
        expect(onAction).toHaveBeenCalledWith('start');
    });

    test('en marcha: métricas, y detener o reiniciar solo tras confirmar', async () => {
        const onAction = vi.fn();
        await render(ServiceControls, {
            name: 'API',
            status: status({
                activeState: 'active',
                subState: 'running',
                pid: 42,
                metrics: { rssBytes: 52_428_800, rssMb: 50, cpuPercent: 12.5 },
            }),
            onAction,
        });
        await expect.element(page.getByText('Activo')).toBeVisible();
        await expect.element(page.getByText(/CPU 12,5 %/)).toBeVisible();
        await button('Detener').click();
        expect(onAction).not.toHaveBeenCalled();
        await page.getByRole('alertdialog').getByRole('button', { name: 'Detener' }).click();
        await expect.poll(() => onAction.mock.calls).toEqual([['stop']]);

        await button('Reiniciar').click();
        await page.getByRole('alertdialog').getByRole('button', { name: 'Reiniciar' }).click();
        await expect.poll(() => onAction.mock.calls.at(-1)).toEqual(['restart']);
    });

    test('con una acción en curso no se puede lanzar otra', async () => {
        await render(ServiceControls, {
            name: 'API',
            status: status({ activeState: 'active' }),
            busy: 'restart',
            onAction: vi.fn(),
        });
        await expect.element(button('Detener')).toBeDisabled();
    });

    test('sitio estático: sin acciones, lo sirve nginx', async () => {
        await render(ServiceControls, { name: 'Web', status: status({}), staticOnly: true, onAction: vi.fn() });
        await expect.element(page.getByText('Sitio estático')).toBeVisible();
        await expect.element(page.getByText('Lo sirve nginx desde la versión activa.')).toBeVisible();
        await expect.element(page.getByRole('button', { name: 'Iniciar' })).not.toBeInTheDocument();
    });
});
