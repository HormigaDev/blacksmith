import { describe, expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import { mockTauri } from '../../../tests/tauri';
import WindowControls from './WindowControls.svelte';

describe('WindowControls', () => {
    test('fuera de Tauri no se muestra', async () => {
        await render(WindowControls);
        await expect.element(page.getByRole('group', { name: 'Controles de la ventana' })).not.toBeInTheDocument();
    });

    test('minimiza, maximiza y cierra con los comandos de ventana', async () => {
        const { calls } = mockTauri({ 'plugin:window|is_maximized': () => false });
        await render(WindowControls);
        await page.getByRole('button', { name: 'Minimizar' }).click();
        await page.getByRole('button', { name: 'Maximizar' }).click();
        await page.getByRole('button', { name: 'Cerrar' }).click();
        await expect
            .poll(() => calls.map((call) => call.cmd).filter((cmd) => cmd !== 'plugin:window|is_maximized'))
            .toEqual(
                expect.arrayContaining([
                    'plugin:window|minimize',
                    'plugin:window|toggle_maximize',
                    'plugin:window|close',
                ]),
            );
    });

    test('maximizada: ofrece restaurar', async () => {
        mockTauri({ 'plugin:window|is_maximized': () => true });
        await render(WindowControls);
        await expect.element(page.getByRole('button', { name: 'Restaurar' })).toBeVisible();
    });
});
