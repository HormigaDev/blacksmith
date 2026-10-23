import { describe, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import FilterSelect from '../FilterSelect.svelte';

const options = [
    { value: 'acme/api', label: 'acme/api' },
    { value: 'acme/web', label: 'acme/web' },
    { value: 'hormiga/blacksmith', label: 'hormiga/blacksmith' },
    { value: 'acme/legacy', label: 'acme/legacy', disabled: true },
];

describe('FilterSelect', () => {
    test('el disparador muestra el placeholder y, después, el valor elegido', async () => {
        const screen = await render(FilterSelect, { label: 'Repositorio', options, placeholder: 'Elige uno' });
        await expect.element(screen.getByRole('button', { name: 'Repositorio' })).toHaveTextContent('Elige uno');
    });

    test('filtra al escribir, sin distinguir mayúsculas, y al elegir vacía la búsqueda', async () => {
        const onValueChange = vi.fn();
        const screen = await render(FilterSelect, { label: 'Repositorio', options, onValueChange });
        await screen.getByRole('button', { name: 'Repositorio' }).click();
        const search = page.getByRole('combobox', { name: 'Buscar en Repositorio' });
        await expect.element(search).toHaveFocus();
        await search.fill('BLACK');
        await expect.element(page.getByRole('option', { name: /hormiga\/blacksmith/ })).toBeVisible();
        await expect.element(page.getByRole('option', { name: /acme\/api/ })).not.toBeInTheDocument();
        await page.getByRole('option', { name: /hormiga\/blacksmith/ }).click();
        expect(onValueChange).toHaveBeenCalledWith('hormiga/blacksmith');
        await expect
            .element(screen.getByRole('button', { name: 'Repositorio' }))
            .toHaveTextContent('hormiga/blacksmith');

        // Al volver a abrir, la búsqueda está vacía y se ven todas.
        await screen.getByRole('button', { name: 'Repositorio' }).click();
        await expect.element(page.getByRole('combobox', { name: 'Buscar en Repositorio' })).toHaveValue('');
        await expect.element(page.getByRole('option', { name: /acme\/api/ })).toBeVisible();
    });

    test('sin coincidencias lo dice', async () => {
        const screen = await render(FilterSelect, { label: 'Repositorio', options });
        await screen.getByRole('button', { name: 'Repositorio' }).click();
        await page.getByRole('combobox', { name: 'Buscar en Repositorio' }).fill('zzz');
        await expect.element(page.getByText('Sin resultados')).toBeVisible();
    });

    test('con teclado: filtra, Enter elige y Escape cierra sin cambiar', async () => {
        const onValueChange = vi.fn();
        const screen = await render(FilterSelect, { label: 'Repositorio', options, value: 'acme/api', onValueChange });
        await screen.getByRole('button', { name: 'Repositorio' }).click();
        await userEvent.keyboard('web{Enter}');
        expect(onValueChange).toHaveBeenCalledWith('acme/web');
        await expect.element(page.getByRole('combobox')).not.toBeInTheDocument();

        onValueChange.mockClear();
        await screen.getByRole('button', { name: 'Repositorio' }).click();
        await userEvent.keyboard('{Escape}');
        await expect.element(page.getByRole('combobox')).not.toBeInTheDocument();
        expect(onValueChange).not.toHaveBeenCalled();
    });

    test('las opciones deshabilitadas no se eligen', async () => {
        const onValueChange = vi.fn();
        const screen = await render(FilterSelect, { label: 'Repositorio', options, onValueChange });
        await screen.getByRole('button', { name: 'Repositorio' }).click();
        await expect
            .element(page.getByRole('option', { name: /acme\/legacy/ }))
            .toHaveAttribute('aria-disabled', 'true');
        await page.getByRole('option', { name: /acme\/legacy/ }).click({ force: true });
        expect(onValueChange).not.toHaveBeenCalled();
    });
});
