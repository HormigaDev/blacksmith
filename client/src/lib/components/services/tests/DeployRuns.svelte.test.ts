import { createRawSnippet } from 'svelte';
import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { DeployRun } from '$lib/agent';

import DeployRuns from '../DeployRuns.svelte';

const NOW = 1_800_000_000_000;
const run = (id: number, status: DeployRun['status'], detail: string | null = null): DeployRun => ({
    id,
    status,
    kind: 'deploy',
    release: `r${String(1_799_999_000 + id).padStart(10, '0')}-000000000`,
    branch: 'main',
    startedAt: NOW / 1000 - 120,
    finishedAt: status === 'running' ? null : NOW / 1000 - 75,
    detail,
});
const output = createRawSnippet(() => ({ render: () => '<p>salida del despliegue</p>' }));

describe('DeployRuns', () => {
    test('lanza un despliegue', async () => {
        const onDeploy = vi.fn();
        await render(DeployRuns, { runs: [], blocked: null, onDeploy, now: NOW });
        await page.getByRole('button', { name: 'Desplegar' }).click();
        expect(onDeploy).toHaveBeenCalledOnce();
    });

    test('bloqueado: explica el motivo y no deja desplegar', async () => {
        await render(DeployRuns, { runs: [], blocked: 'Define el comando de arranque.', onDeploy: vi.fn(), now: NOW });
        await expect.element(page.getByRole('button', { name: 'Desplegar' })).toBeDisabled();
        await expect.element(page.getByText('Define el comando de arranque.')).toBeVisible();
    });

    test('último fallido: aviso sobrio y la salida a petición', async () => {
        await render(DeployRuns, {
            runs: [run(3, 'failed', 'el despliegue falló: revisa sus logs'), run(2, 'succeeded')],
            blocked: null,
            onDeploy: vi.fn(),
            output,
            now: NOW,
        });
        const latest = page.getByRole('region', { name: 'Último despliegue' });
        await expect.element(latest.getByText('Fallido')).toBeVisible();
        await expect.element(latest.getByText(/el despliegue falló/)).toBeVisible();
        await expect.element(page.getByText('salida del despliegue')).not.toBeInTheDocument();
        await page.getByRole('button', { name: 'Ver salida' }).click();
        await expect.element(page.getByText('salida del despliegue')).toBeVisible();
        await expect
            .element(page.getByRole('button', { name: 'Ocultar salida' }))
            .toHaveAttribute('aria-expanded', 'true');
    });

    test('en curso: la salida se ve sola y no se puede lanzar otro', async () => {
        await render(DeployRuns, {
            runs: [run(4, 'running'), run(3, 'failed', 'npm ci falló'), run(2, 'succeeded')],
            blocked: null,
            onDeploy: vi.fn(),
            output,
            now: NOW,
        });
        await expect.element(page.getByRole('button', { name: /Desplegando/ })).toBeDisabled();
        await expect.element(page.getByText('salida del despliegue')).toBeVisible();
        // Los anteriores, compactos: estado con etiqueta y sin el motivo de cada fallo.
        const history = page.getByRole('region', { name: 'Anteriores' });
        await expect.element(history.getByRole('listitem').nth(1)).toBeInTheDocument();
        expect(history.getByRole('listitem').nth(0).element().textContent).toContain('Fallido');
        expect(history.getByRole('listitem').nth(1).element().textContent).toContain('Desplegado');
        await expect.element(page.getByText('npm ci falló')).not.toBeInTheDocument();
    });

    test('rollback: marca la versión activa y vuelve a la anterior tras confirmar', async () => {
        const onRollback = vi.fn(async () => {});
        const runs = [run(2, 'succeeded'), { ...run(1, 'succeeded'), kind: 'rollback' as const }];
        await render(DeployRuns, {
            runs,
            blocked: null,
            activeRelease: runs[0]?.release,
            previousRelease: runs[1]?.release,
            onDeploy: vi.fn(),
            onRollback,
            now: NOW,
        });
        await expect.element(page.getByRole('region', { name: 'Último despliegue' }).getByText('En uso')).toBeVisible();
        expect(page.getByRole('region', { name: 'Anteriores' }).element().textContent).toContain('Rollback');

        await page.getByRole('button', { name: 'Volver a la anterior' }).click();
        expect(onRollback).not.toHaveBeenCalled();
        await page.getByRole('alertdialog').getByRole('button', { name: 'Volver a la anterior' }).click();
        await expect.poll(() => onRollback.mock.calls.length).toBe(1);
    });

    test('sin versión anterior no ofrece rollback', async () => {
        await render(DeployRuns, {
            runs: [run(1, 'succeeded')],
            blocked: null,
            previousRelease: null,
            onDeploy: vi.fn(),
            onRollback: vi.fn(),
            now: NOW,
        });
        await expect.element(page.getByRole('button', { name: 'Volver a la anterior' })).not.toBeInTheDocument();
    });

    test('versión en uso con el servicio detenido: lo aclara', async () => {
        const runs = [run(2, 'succeeded')];
        await render(DeployRuns, {
            runs,
            blocked: null,
            activeRelease: runs[0]?.release,
            serviceRunning: false,
            onDeploy: vi.fn(),
            now: NOW,
        });
        await expect.element(page.getByText(/el servicio está detenido/)).toBeVisible();
    });
});
