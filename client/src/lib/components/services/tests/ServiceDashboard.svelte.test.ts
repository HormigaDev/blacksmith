import { describe, expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { ServiceStatus } from '$lib/agent';

import ServiceDashboard from '../ServiceDashboard.svelte';

const GIB = 1024 ** 3;
const NOW = 1_800_000_000_000;
const status = (overrides: Partial<ServiceStatus['resources']> = {}, active = true): ServiceStatus => ({
    name: 'api',
    unit: 'blacksmith@api.service',
    loadState: 'loaded',
    activeState: active ? 'active' : 'inactive',
    subState: active ? 'running' : 'dead',
    pid: active ? 1234 : null,
    metrics: null,
    deployed: true,
    release: 'r1799990000-000000000',
    previousRelease: null,
    resources: {
        memoryBytes: 512 * 1024 ** 2,
        memoryPeakBytes: 640 * 1024 ** 2,
        memoryLimitBytes: null,
        swapBytes: 0,
        cpuPercent: 12.5,
        tasks: 7,
        restarts: 0,
        uptimeSecs: 2 * 3600 + 300,
        ...overrides,
    },
    host: { cpuCores: 4, memoryBytes: 8 * GIB, swapBytes: 2 * GIB },
});
const history = [
    { at: NOW - 6000, cpu: 10, memory: 500 * 1024 ** 2 },
    { at: NOW - 3000, cpu: 12.5, memory: 512 * 1024 ** 2 },
];

describe('ServiceDashboard', () => {
    test('consumo del servicio de un vistazo', async () => {
        await render(ServiceDashboard, { status: status(), history, branch: 'main', site: null, now: NOW });
        await expect
            .element(page.getByRole('region', { name: 'CPU' }).getByText('12,5 %', { exact: true }))
            .toBeVisible();
        await expect
            .element(page.getByRole('region', { name: 'CPU' }).getByText('4 en el servidor', { exact: false }))
            .toBeVisible();
        await expect
            .element(page.getByRole('region', { name: 'Procesos e hilos' }).getByText('7', { exact: false }))
            .toBeVisible();
        await expect
            .element(page.getByRole('region', { name: 'Swap' }).getByText('De 2 GB en el servidor', { exact: false }))
            .toBeVisible();
        await expect
            .element(page.getByRole('region', { name: 'Versión activa' }).getByText('Rama main', { exact: false }))
            .toBeVisible();
        await expect
            .element(page.getByRole('region', { name: 'Web' }).getByText('Sin sitio en nginx', { exact: false }))
            .toBeVisible();
        // Memoria frente a la RAM del servidor (sin límite propio).
        await expect
            .element(page.getByRole('meter', { name: /Memoria frente a la RAM del servidor/ }))
            .toHaveAttribute('aria-valuenow', String(512 * 1024 ** 2));
        await expect.element(page.getByRole('slider', { name: 'CPU en los últimos minutos' })).toBeVisible();
    });

    test('con límite de memoria, el medidor es frente a ese límite', async () => {
        await render(ServiceDashboard, {
            status: status({ memoryLimitBytes: GIB }),
            history,
            now: NOW,
        });
        await expect.element(page.getByRole('meter', { name: /frente a su límite de 1 GB/ })).toBeVisible();
    });

    test('parado: lo dice y no inventa consumo', async () => {
        await render(ServiceDashboard, {
            status: status(
                { memoryBytes: null, cpuPercent: null, tasks: null, uptimeSecs: null, swapBytes: null },
                false,
            ),
            history: [],
            now: NOW,
        });
        await expect.element(page.getByRole('status').getByText(/no está en marcha/)).toBeVisible();
        await expect.element(page.getByRole('region', { name: 'CPU' }).getByText('—', { exact: false })).toBeVisible();
        await expect.element(page.getByRole('meter')).not.toBeInTheDocument();
    });
});
