import { describe, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import { HttpError } from '$lib/http';

import EnvEditor from '../EnvEditor.svelte';

const keys = () => page.getByRole('textbox', { name: /^Clave/ });
const save = () => page.getByRole('button', { name: 'Guardar' });

describe('EnvEditor', () => {
    test('solo guarda con cambios, y envía claves limpias', async () => {
        const onSave = vi.fn(async () => {});
        await render(EnvEditor, { variables: [{ key: 'A', value: '1' }], onSave });
        await expect.element(save()).toBeDisabled();
        await page.getByRole('button', { name: 'Añadir variable' }).click();
        await keys().nth(1).fill(' NODE_ENV ');
        await page
            .getByLabelText(/^Valor/)
            .nth(1)
            .fill('production');
        await save().click();
        expect(onSave).toHaveBeenCalledWith([
            { key: 'A', value: '1' },
            { key: 'NODE_ENV', value: 'production' },
        ]);
        await expect.element(save()).toBeDisabled();
    });

    test('valida claves inválidas y repetidas antes de enviar', async () => {
        const onSave = vi.fn(async () => {});
        await render(EnvEditor, { variables: [{ key: 'A', value: '1' }], onSave });
        await page.getByRole('button', { name: 'Añadir variable' }).click();
        await keys().nth(1).fill('A');
        await save().click();
        await expect.element(keys().nth(1)).toHaveAccessibleDescription(/Clave repetida/);
        await keys().nth(1).fill('1MAL');
        await expect.element(keys().nth(1)).toHaveAccessibleDescription(/sin empezar por un dígito/);
        expect(onSave).not.toHaveBeenCalled();
    });

    test('los valores van ocultos; quitar y descartar', async () => {
        await render(EnvEditor, { variables: [{ key: 'TOKEN', value: 'secreto' }], onSave: vi.fn() });
        await expect.element(page.getByLabelText(/^Valor/)).toHaveAttribute('type', 'password');
        await page.getByRole('button', { name: 'Quitar TOKEN' }).click();
        await expect.element(page.getByText('Sin variables.')).toBeVisible();
        await page.getByRole('button', { name: 'Descartar cambios' }).click();
        await expect.element(keys()).toHaveValue('TOKEN');
    });

    test('un error del agente se muestra y conserva lo escrito', async () => {
        const onSave = vi.fn(async () => {
            throw new HttpError(400, 'INVALID_ENVIRONMENT', 'x', 'PUT', '/', 'r');
        });
        await render(EnvEditor, { variables: [], onSave });
        await page.getByRole('button', { name: 'Añadir variable' }).click();
        await keys().fill('A');
        await userEvent.click(save());
        await expect.element(page.getByRole('alert')).toHaveTextContent('Alguna variable de entorno no es válida.');
        await expect.element(keys()).toHaveValue('A');
    });

    test('alcance de servicio: lista las globales y marca las sobrescritas', async () => {
        await render(EnvEditor, {
            variables: [{ key: 'PORT', value: '8080' }],
            inherited: [
                { key: 'PORT', value: '80' },
                { key: 'NODE_ENV', value: 'production' },
            ],
            onSave: vi.fn(),
        });
        await expect.element(page.getByRole('heading', { name: 'Heredadas del servidor' })).toBeVisible();
        const items = page.getByRole('listitem').filter({ hasText: 'PORT' });
        await expect.element(items.getByText('Sobrescrita')).toBeVisible();
        expect(page.getByText('production').query()).toBeNull();
    });
});
