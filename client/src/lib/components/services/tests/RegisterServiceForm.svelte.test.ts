import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import { HttpError } from '$lib/http';

import RegisterServiceForm from '../RegisterServiceForm.svelte';

const identifier = () => page.getByRole('textbox', { name: /^Identificador/ });
const displayName = () => page.getByRole('textbox', { name: /^Nombre/ });
const submit = () => page.getByRole('button', { name: 'Registrar' });

describe('RegisterServiceForm', () => {
    test('envía los datos limpios y muestra la unidad que define', async () => {
        const onSubmit = vi.fn(async () => {});
        await render(RegisterServiceForm, { onSubmit });
        await identifier().fill(' mi-api ');
        await expect
            .element(page.getByText('Minúsculas, dígitos y guiones. Define la unidad blacksmith-mi-api.service.'))
            .toBeVisible();
        await displayName().fill('API de pagos');
        await page.getByRole('textbox', { name: /^Descripción/ }).fill('  Cobros.  ');
        await submit().click();
        expect(onSubmit).toHaveBeenCalledWith({ name: 'mi-api', displayName: 'API de pagos', description: 'Cobros.' });
    });

    test('valida antes de enviar', async () => {
        const onSubmit = vi.fn(async () => {});
        await render(RegisterServiceForm, { onSubmit });
        await submit().click();
        await expect.element(identifier()).toHaveAccessibleDescription(/Obligatorio/);
        await expect.element(displayName()).toHaveAccessibleDescription(/Obligatorio/);
        await identifier().fill('Mi_API');
        await submit().click();
        await expect.element(identifier()).toHaveAccessibleDescription(/minúsculas, dígitos o guiones/);
        expect(onSubmit).not.toHaveBeenCalled();
    });

    test('un identificador repetido se marca en su campo; otros errores, en el formulario', async () => {
        const onSubmit = vi
            .fn<() => Promise<void>>()
            .mockRejectedValueOnce(new HttpError(409, 'SERVICE_ALREADY_EXISTS', 'x', 'POST', '/', 'r'))
            .mockRejectedValueOnce(new HttpError(500, 'INTERNAL', 'x', 'POST', '/', 'r'));
        await render(RegisterServiceForm, { onSubmit });
        await identifier().fill('api');
        await displayName().fill('API');
        await submit().click();
        await expect.element(identifier()).toHaveAccessibleDescription(/Ya hay un servicio registrado/);
        await submit().click();
        await expect.element(page.getByRole('alert')).toHaveTextContent('El agente encontró un error interno.');
    });
});
