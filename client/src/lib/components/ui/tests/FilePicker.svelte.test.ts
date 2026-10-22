import { describe, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { mockTauri } from '../../../../tests/tauri';
import FilePicker from '../FilePicker.svelte';

describe('FilePicker (Tauri simulado)', () => {
    test('abre el diálogo nativo con los filtros y muestra la ruta elegida', async () => {
        const { calls } = mockTauri({ 'plugin:dialog|open': () => '/home/ana/.ssh/deploy.pem' });
        const onValueChange = vi.fn();
        const screen = await render(FilePicker, { label: 'Llave SSH', accept: ['pem', 'key'], onValueChange });

        await screen.getByRole('button', { name: 'Seleccionar archivo' }).click();
        await expect.element(screen.getByText('deploy.pem')).toBeVisible();
        await expect.element(screen.getByText('/home/ana/.ssh')).toBeVisible();
        expect(onValueChange).toHaveBeenCalledWith(['/home/ana/.ssh/deploy.pem']);

        const open = calls.find((call) => call.cmd === 'plugin:dialog|open');
        expect(open?.args).toMatchObject({
            options: { multiple: false, directory: false, filters: [{ extensions: ['pem', 'key'] }] },
        });
    });

    test('rechaza formatos no admitidos y lo anuncia', async () => {
        mockTauri({ 'plugin:dialog|open': () => ['/tmp/a.pem', '/tmp/notas.txt', '/tmp/b.exe'] });
        const screen = await render(FilePicker, { label: 'Llaves', accept: ['pem'], multiple: true });
        await screen.getByRole('button', { name: 'Seleccionar archivos' }).click();
        await expect.element(screen.getByRole('alert')).toHaveTextContent('2 archivos no tienen un formato admitido');
        await expect.element(screen.getByText('a.pem')).toBeVisible();
    });

    test('quita un archivo con su botón accesible', async () => {
        mockTauri({});
        const screen = await render(FilePicker, { label: 'Llave', value: ['/tmp/a.pem'] });
        await screen.getByRole('button', { name: 'Quitar a.pem' }).click();
        await expect.element(screen.getByText('a.pem')).not.toBeInTheDocument();
    });

    test('cancelar el diálogo no cambia nada', async () => {
        mockTauri({ 'plugin:dialog|open': () => null });
        const onValueChange = vi.fn();
        const screen = await render(FilePicker, { label: 'Llave', onValueChange });
        await screen.getByRole('button', { name: 'Seleccionar archivo' }).click();
        expect(onValueChange).not.toHaveBeenCalled();
    });
});
