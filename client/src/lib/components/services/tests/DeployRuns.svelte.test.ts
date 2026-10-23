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

    test('lista los últimos con su resultado; uno en curso bloquea el botón', async () => {
        await render(DeployRuns, {
            runs: [run(3, 'running'), run(2, 'failed', 'npm ci terminó con código 1'), run(1, 'succeeded')],
            blocked: null,
            onDeploy: vi.fn(),
            now: NOW,
        });
        await expect.element(page.getByRole('button', { name: /Desplegando/ })).toBeDisabled();
        const items = page.getByRole('listitem');
        await expect.element(items.nth(2)).toBeInTheDocument();
        const text = (index: number) => items.nth(index).element().textContent ?? '';
        expect(text(0)).toContain('En curso');
        expect(text(1)).toContain('Fallido');
        expect(text(1)).toContain('npm ci terminó con código 1');
        expect(text(2)).toContain('Correcto');
        expect(text(2)).toContain('45 s');
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
        const items = page.getByRole('listitem');
        await expect.element(items.nth(1)).toBeInTheDocument();
        expect(items.nth(0).element().textContent).toContain('Activa');
        expect(items.nth(1).element().textContent).toContain('Rollback');

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
});
