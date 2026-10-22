import { describe, expect, test } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import Input from '../Input.svelte';

describe('Input', () => {
    test('la etiqueta, la ayuda y el error quedan enlazados por ARIA', async () => {
        const screen = await render(Input, {
            label: 'Host',
            hint: 'Nombre o IP',
            error: 'Host inválido',
            required: true,
        });
        const input = screen.getByRole('textbox', { name: /Host/ });
        await expect.element(input).toHaveAttribute('aria-invalid', 'true');
        await expect.element(input).toHaveAccessibleDescription('Host inválido');
        await expect.element(input).toBeRequired();
        // Con error, la ayuda no se muestra: el error la sustituye.
        await expect.element(screen.getByText('Nombre o IP')).not.toBeInTheDocument();
    });

    test('password: alterna entre oculta y visible', async () => {
        const screen = await render(Input, { label: 'Contraseña', type: 'password', value: 'secreto' });
        const field = screen.getByLabelText('Contraseña', { exact: true });
        await expect.element(field).toHaveAttribute('type', 'password');
        await screen.getByRole('button', { name: 'Mostrar contraseña' }).click();
        await expect.element(field).toHaveAttribute('type', 'text');
        await expect
            .element(screen.getByRole('button', { name: 'Ocultar contraseña' }))
            .toHaveAttribute('aria-pressed', 'true');
    });

    describe('number', () => {
        test('los controles aparecen solo con el foco y no lo roban', async () => {
            const screen = await render(Input, { label: 'Puerto', type: 'number', value: 22 });
            const input = screen.getByRole('spinbutton', { name: 'Puerto' });
            const increment = screen.getByRole('button', { name: 'Aumentar' });
            // Ocultos (visibility: hidden) quedan fuera del árbol de accesibilidad.
            await expect.element(increment).not.toBeInTheDocument();

            await input.click();
            await expect.element(increment).toBeVisible();
            await increment.click();
            await increment.click();
            await screen.getByRole('button', { name: 'Disminuir' }).click();
            await expect.element(input).toHaveValue(23);
            await expect.element(input).toHaveFocus();
        });

        test('respeta min y max', async () => {
            const screen = await render(Input, { label: 'Puerto', type: 'number', value: 65535, min: 1, max: 65535 });
            await screen.getByRole('spinbutton').click();
            await expect.element(screen.getByRole('button', { name: 'Aumentar' })).toBeDisabled();
            await expect.element(screen.getByRole('button', { name: 'Disminuir' })).toBeEnabled();
        });

        test('las flechas del teclado siguen funcionando', async () => {
            const screen = await render(Input, { label: 'Réplicas', type: 'number', value: 1 });
            await screen.getByRole('spinbutton').click();
            await userEvent.keyboard('{ArrowUp}{ArrowUp}');
            await expect.element(screen.getByRole('spinbutton')).toHaveValue(3);
        });

        test('deshabilitado: sin controles', async () => {
            const screen = await render(Input, { label: 'Puerto', type: 'number', value: 1, disabled: true });
            await expect.element(screen.getByRole('button', { name: 'Aumentar' })).not.toBeInTheDocument();
        });
    });
});
