import { describe, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import ConfirmDialog from '../ConfirmDialog.svelte';

const base = { open: true, title: '¿Detener api.service?', description: 'Dejará de atender peticiones.' };

describe('ConfirmDialog', () => {
    test('es un alertdialog accesible con su título y descripción', async () => {
        await render(ConfirmDialog, { ...base, onConfirm: vi.fn() });
        const dialog = page.getByRole('alertdialog', { name: base.title });
        await expect.element(dialog).toBeVisible();
        await expect.element(dialog).toHaveAccessibleDescription(base.description);
    });

    test('confirma, muestra la carga y se cierra al terminar', async () => {
        let finish: () => void = () => {};
        const onConfirm = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
        await render(ConfirmDialog, { ...base, variant: 'danger', confirmLabel: 'Detener', onConfirm });

        await page.getByRole('button', { name: 'Detener' }).click();
        expect(onConfirm).toHaveBeenCalledOnce();
        await expect.element(page.getByRole('button', { name: /Cargando/ })).toHaveAttribute('aria-busy', 'true');
        // Mientras trabaja no se puede cerrar ni cancelar.
        await userEvent.keyboard('{Escape}');
        await expect.element(page.getByRole('alertdialog')).toBeVisible();
        await expect.element(page.getByRole('button', { name: 'Cancelar' })).toBeDisabled();

        finish();
        await expect.element(page.getByRole('alertdialog')).not.toBeInTheDocument();
    });

    test('si la acción falla, sigue abierto para reintentar', async () => {
        const onConfirm = vi.fn(async () => {
            throw new Error('agente caído');
        });
        // El error se propaga al llamador (que decide cómo avisar); aquí solo se observa.
        const unhandled = vi.fn();
        window.addEventListener('unhandledrejection', (event) => {
            unhandled();
            event.preventDefault();
        });
        await render(ConfirmDialog, { ...base, onConfirm });
        await page.getByRole('button', { name: 'Confirmar' }).click();
        await expect.element(page.getByRole('button', { name: 'Confirmar' })).toBeEnabled();
        await expect.element(page.getByRole('alertdialog')).toBeVisible();
    });

    test('cancelar no ejecuta la acción', async () => {
        const onConfirm = vi.fn();
        await render(ConfirmDialog, { ...base, onConfirm });
        await page.getByRole('button', { name: 'Cancelar' }).click();
        await expect.element(page.getByRole('alertdialog')).not.toBeInTheDocument();
        expect(onConfirm).not.toHaveBeenCalled();
    });
});
