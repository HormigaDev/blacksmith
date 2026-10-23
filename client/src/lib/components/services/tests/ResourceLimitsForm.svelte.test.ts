import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { ServiceLimits } from '$lib/agent';

import ResourceLimitsForm from '../ResourceLimitsForm.svelte';

const MIB = 1024 ** 2;
const GIB = 1024 * MIB;
const LIMITS: ServiceLimits = {
    limits: { cpuPercent: null, priority: 'normal', memoryMaxBytes: null, swapMaxBytes: null, tasksMax: null },
    host: { cpuCores: 2, memoryBytes: 4 * GIB, swapBytes: GIB },
    updatedAt: null,
};

const save = () => page.getByRole('button', { name: 'Aplicar límites' }).click();

describe('ResourceLimitsForm', () => {
    test('sin tocar nada guarda «sin límites»', async () => {
        const onSave = vi.fn(async () => {});
        await render(ResourceLimitsForm, { limits: LIMITS, onSave });
        await save();
        expect(onSave).toHaveBeenCalledWith({
            cpuPercent: null,
            priority: 'normal',
            memoryMaxBytes: null,
            swapMaxBytes: null,
            tasksMax: null,
        });
    });

    test('CPU, prioridad, memoria en GB, sin swap y procesos', async () => {
        const onSave = vi.fn(async () => {});
        await render(ResourceLimitsForm, { limits: LIMITS, onSave });
        await page.getByRole('spinbutton', { name: 'CPU máxima' }).fill('150');
        await page.getByRole('radio', { name: 'Alta' }).click();
        await page.getByRole('spinbutton', { name: 'Memoria máxima' }).fill('1.5');
        await page.getByRole('button', { name: 'Unidad' }).first().click();
        await page.getByRole('option', { name: 'GB' }).click();
        await page.getByRole('radio', { name: 'Sin swap' }).click();
        await page.getByRole('spinbutton', { name: /Procesos e hilos/ }).fill('256');
        await save();
        expect(onSave).toHaveBeenCalledWith({
            cpuPercent: 150,
            priority: 'high',
            memoryMaxBytes: 1.5 * GIB,
            swapMaxBytes: 0,
            tasksMax: 256,
        });
    });

    test('valida como el agente: CPU hasta los núcleos, memoria mínima y procesos', async () => {
        const onSave = vi.fn(async () => {});
        await render(ResourceLimitsForm, { limits: LIMITS, onSave });
        await page.getByRole('spinbutton', { name: 'CPU máxima' }).fill('250');
        await page.getByRole('spinbutton', { name: 'Memoria máxima' }).fill('16');
        await page.getByRole('spinbutton', { name: /Procesos e hilos/ }).fill('4');
        await save();
        await expect
            .element(page.getByRole('spinbutton', { name: 'CPU máxima' }))
            .toHaveAccessibleDescription(/Entre 1 % y 200 %/);
        await expect
            .element(page.getByRole('spinbutton', { name: 'Memoria máxima' }))
            .toHaveAccessibleDescription(/Entre 32 MB/);
        await expect
            .element(page.getByRole('spinbutton', { name: /Procesos e hilos/ }))
            .toHaveAccessibleDescription(/Entre 16/);
        expect(onSave).not.toHaveBeenCalled();
    });

    test('swap con límite, y sin swap en el servidor no se ofrece', async () => {
        const onSave = vi.fn(async () => {});
        const { unmount } = await render(ResourceLimitsForm, { limits: LIMITS, onSave });
        await page.getByRole('radio', { name: 'Hasta…' }).click();
        await page.getByRole('spinbutton', { name: 'Swap máxima' }).fill('256');
        await save();
        expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ swapMaxBytes: 256 * MIB }));
        unmount();

        await render(ResourceLimitsForm, { limits: { ...LIMITS, host: { ...LIMITS.host, swapBytes: 0 } }, onSave });
        await expect.element(page.getByRole('radio', { name: 'Hasta…' })).toBeDisabled();
        await expect.element(page.getByText('El servidor no tiene swap.')).toBeVisible();
    });

    test('muestra los límites guardados y el consumo; «Quitar todos» los vacía', async () => {
        const onSave = vi.fn(async () => {});
        await render(ResourceLimitsForm, {
            limits: {
                ...LIMITS,
                limits: { cpuPercent: 50, priority: 'low', memoryMaxBytes: GIB, swapMaxBytes: null, tasksMax: 64 },
                updatedAt: 1,
            },
            usage: {
                memoryBytes: 300 * MIB,
                memoryPeakBytes: 400 * MIB,
                memoryLimitBytes: GIB,
                swapBytes: null,
                swapLimitBytes: null,
                cpuPercent: 12,
                cpuLimitPercent: 50,
                tasks: 9,
                tasksLimit: 64,
                restarts: 0,
                uptimeSecs: 10,
            },
            onSave,
        });
        await expect.element(page.getByRole('spinbutton', { name: 'CPU máxima' })).toHaveValue(50);
        await expect.element(page.getByRole('spinbutton', { name: 'Memoria máxima' })).toHaveValue(1);
        await expect.element(page.getByText(/Ahora usa 300 MB \(pico 400 MB\)/)).toBeVisible();
        await page.getByRole('button', { name: 'Quitar todos' }).click();
        await save();
        expect(onSave).toHaveBeenCalledWith({
            cpuPercent: null,
            priority: 'normal',
            memoryMaxBytes: null,
            swapMaxBytes: null,
            tasksMax: null,
        });
    });

    test('un error del agente se muestra en el formulario', async () => {
        const onSave = vi.fn(async () => {
            throw new Error('sin permiso');
        });
        await render(ResourceLimitsForm, { limits: LIMITS, onSave });
        await save();
        await expect.element(page.getByRole('alert')).toBeVisible();
    });
});
