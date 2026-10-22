import { describe, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import Select from '../Select.svelte';

const options = [
    { value: 'nginx', label: 'nginx.service' },
    { value: 'api', label: 'api.service' },
    { value: 'legacy', label: 'legacy.service', disabled: true },
];

// bits-ui expone el disparador como `button` con `aria-haspopup="listbox"`.
describe('Select', () => {
    test('muestra el placeholder traducido sin valor', async () => {
        const screen = await render(Select, { label: 'Unidad', options });
        await expect.element(screen.getByRole('button', { name: 'Unidad' })).toHaveTextContent('Selecciona una opción');
    });

    test('elige una opción con el ratón', async () => {
        const onValueChange = vi.fn();
        const screen = await render(Select, { label: 'Unidad', options, onValueChange });
        await screen.getByRole('button', { name: 'Unidad' }).click();
        // El menú se renderiza en un portal, fuera del contenedor del componente.
        await page.getByRole('option', { name: 'api.service' }).click();
        expect(onValueChange).toHaveBeenCalledWith('api');
        await expect.element(screen.getByRole('button', { name: 'Unidad' })).toHaveTextContent('api.service');
    });

    test('se maneja con teclado y omite las opciones deshabilitadas', async () => {
        const onValueChange = vi.fn();
        const screen = await render(Select, { label: 'Unidad', options, value: 'api', onValueChange });
        await screen.getByRole('button', { name: 'Unidad' }).click();
        await expect.element(page.getByRole('option', { name: 'legacy.service' })).toHaveAttribute('data-disabled');
        await userEvent.keyboard('{ArrowUp}{Enter}');
        expect(onValueChange).toHaveBeenCalledWith('nginx');
    });
});
