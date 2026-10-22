import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import { mockTauri } from '../../../tests/tauri';
import AddServerForm from './AddServerForm.svelte';

const PEM = '-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n-----END OPENSSH PRIVATE KEY-----';
const SERVER = {
    id: 'deploy@vps.example.com:22',
    kind: 'ssh' as const,
    host: 'vps.example.com',
    port: 22,
    username: 'deploy',
};
const LOCAL = { id: 'local:4141', kind: 'local' as const, host: 'localhost', port: 4141, username: '' };
const SAVED = { server: SERVER, key: { algorithm: 'ssh-ed25519', fingerprint: 'SHA256:x', encrypted: false } };

const rejection = (code: string) => {
    throw { code, message: code };
};

function sequence(...replies: unknown[]) {
    return () => {
        const reply = replies.length > 1 ? replies.shift() : replies[0];
        return typeof reply === 'function' ? (reply as () => unknown)() : reply;
    };
}

async function fill() {
    await page.getByRole('textbox', { name: /Host/ }).fill('VPS.example.com');
    await page.getByRole('textbox', { name: /Usuario/ }).fill('deploy');
    await page.getByRole('radio', { name: 'Pegar' }).click();
    await page.getByRole('textbox', { name: /Contenido de la llave/ }).fill(PEM);
}

const submit = () => page.getByRole('button', { name: 'Guardar y conectar' }).click();

describe('AddServerForm', () => {
    test('valida los campos obligatorios sin llamar a Rust', async () => {
        const { calls } = mockTauri({});
        await render(AddServerForm, {});
        await submit();
        await expect.element(page.getByRole('textbox', { name: /Host/ })).toHaveAttribute('aria-invalid', 'true');
        await expect.element(page.getByRole('textbox', { name: /Usuario/ })).toHaveAttribute('aria-invalid', 'true');
        expect(calls.some((call) => call.cmd === 'ssh_save_key')).toBe(false);
    });

    test('guarda el servidor con el perfil normalizado por Rust y olvida la llave pegada', async () => {
        const { calls } = mockTauri({ ssh_save_key: () => SAVED });
        const onSaved = vi.fn();
        await render(AddServerForm, { onSaved });
        await fill();
        await submit();

        await expect.poll(() => onSaved.mock.calls.length).toBe(1);
        expect(onSaved).toHaveBeenCalledWith(SERVER, undefined);
        expect(calls.find((call) => call.cmd === 'ssh_save_key')?.args).toMatchObject({
            target: { host: 'VPS.example.com', port: 22, username: 'deploy' },
            source: { kind: 'inline', content: PEM },
        });
        // El formulario solo da de alta: conectar es cosa del estado de servidores.
        expect(calls.some((call) => call.cmd === 'tunnel_open')).toBe(false);
        await expect.element(page.getByRole('textbox', { name: /Contenido de la llave/ })).toHaveValue('');
    });

    test('pide la passphrase y la entrega para la primera conexión', async () => {
        const { calls } = mockTauri({ ssh_save_key: sequence(() => rejection('PASSPHRASE_REQUIRED'), SAVED) });
        const onSaved = vi.fn();
        await render(AddServerForm, { onSaved });
        await fill();
        await submit();

        const passphrase = page.getByLabelText('Passphrase de la llave', { exact: true });
        await expect.element(passphrase).toHaveAccessibleDescription('La llave está protegida. Escribe su passphrase.');
        await passphrase.fill('correct horse battery staple');
        await page.getByText('Recordar la passphrase en este equipo').click();
        await submit();

        await expect.poll(() => onSaved.mock.calls.length).toBe(1);
        expect(onSaved).toHaveBeenCalledWith(SERVER, 'correct horse battery staple');
        expect(calls.filter((call) => call.cmd === 'ssh_save_key').at(-1)?.args).toMatchObject({
            rememberPassphrase: true,
        });
    });

    test('señala la llave cuando no es válida', async () => {
        mockTauri({ ssh_save_key: () => rejection('KEY_IS_PUBLIC') });
        const onSaved = vi.fn();
        await render(AddServerForm, { onSaved });
        await fill();
        await submit();
        await expect
            .element(page.getByRole('textbox', { name: /Contenido de la llave/ }))
            .toHaveAccessibleDescription('Esa es la llave pública (.pub). Hace falta la privada.');
        expect(onSaved).not.toHaveBeenCalled();
    });

    describe('localhost (este equipo)', () => {
        test('no pide usuario ni llave y registra el servidor local, sin SSH', async () => {
            const { calls } = mockTauri({ server_add_local: () => LOCAL });
            const onSaved = vi.fn();
            await render(AddServerForm, { onSaved });
            await page.getByRole('textbox', { name: /Host/ }).fill('localhost');

            await expect.element(page.getByText(/no hace falta túnel SSH ni llave/)).toBeVisible();
            await expect.element(page.getByRole('textbox', { name: /Usuario/ })).not.toBeInTheDocument();
            await expect.element(page.getByRole('radio', { name: 'Pegar' })).not.toBeInTheDocument();

            const port = page.getByRole('spinbutton', { name: /Puerto del agente/ });
            await expect.element(port).toHaveValue(4040);
            await port.fill('4141');
            await submit();

            await expect.poll(() => onSaved.mock.calls.length).toBe(1);
            expect(onSaved).toHaveBeenCalledWith(LOCAL, undefined);
            expect(calls.find((call) => call.cmd === 'server_add_local')?.args).toEqual({ port: 4141 });
            expect(calls.some((call) => call.cmd === 'ssh_save_key')).toBe(false);
        });

        test('valida el puerto del agente', async () => {
            const { calls } = mockTauri({});
            await render(AddServerForm, { initial: { host: '127.0.0.1' } });
            await page.getByRole('spinbutton', { name: /Puerto del agente/ }).fill('70000');
            await submit();
            await expect
                .element(page.getByRole('spinbutton', { name: /Puerto del agente/ }))
                .toHaveAttribute('aria-invalid', 'true');
            expect(calls.some((call) => call.cmd === 'server_add_local')).toBe(false);
        });
    });
});
