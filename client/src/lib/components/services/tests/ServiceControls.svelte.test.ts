import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { ServiceStatus } from '$lib/agent';

import ServiceControls from '../ServiceControls.svelte';

const status = (overrides: Partial<ServiceStatus>): ServiceStatus => ({
    name: 'api',
    unit: 'blacksmith-api.service',
    loadState: 'loaded',
    activeState: 'inactive',
    subState: 'dead',
    pid: null,
    metrics: null,
    deployed: true,
    release: 'r1800000000-000000000',
    previousRelease: null,
    resources: {
        memoryBytes: null,
        memoryPeakBytes: null,
        memoryLimitBytes: null,
        swapLimitBytes: null,
        cpuLimitPercent: null,
        tasksLimit: null,
        swapBytes: null,
        cpuPercent: null,
        tasks: null,
        restarts: null,
        uptimeSecs: null,
    },
    host: { cpuCores: 4, memoryBytes: 8 * 1024 ** 3, swapBytes: 0 },
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
                // El proceso principal (npm) usa poco; el servicio entero, más: se muestra el servicio.
                metrics: { rssBytes: 10_485_760, rssMb: 10, cpuPercent: 1 },
                resources: {
                    memoryBytes: 52_428_800,
                    memoryPeakBytes: null,
                    memoryLimitBytes: null,
                    swapLimitBytes: null,
                    cpuLimitPercent: null,
                    tasksLimit: null,
                    swapBytes: 0,
                    cpuPercent: 12.5,
                    tasks: 9,
                    restarts: 0,
                    uptimeSecs: 30,
                },
            }),
            onAction,
        });
        await expect.element(page.getByText('Activo')).toBeVisible();
        await expect.element(page.getByText('CPU 12,5 % · Memoria 50 MB')).toBeVisible();
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
