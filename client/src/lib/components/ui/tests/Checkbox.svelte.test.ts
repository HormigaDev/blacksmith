import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import Checkbox from '../Checkbox.svelte';
import Switch from '../Switch.svelte';

test('Checkbox: se marca al pulsar su etiqueta', async () => {
    const onCheckedChange = vi.fn();
    const screen = await render(Checkbox, { label: 'Acepto', description: 'Reinicia el servicio', onCheckedChange });
    const box = screen.getByRole('checkbox', { name: 'Acepto' });
    await expect.element(box).toHaveAccessibleDescription('Reinicia el servicio');
    await screen.getByText('Acepto').click();
    await expect.element(box).toBeChecked();
    expect(onCheckedChange).toHaveBeenCalledWith(true);
});

test('Checkbox: estado mixto', async () => {
    const screen = await render(Checkbox, { label: 'Todos', indeterminate: true });
    await expect.element(screen.getByRole('checkbox', { name: 'Todos' })).toHaveAttribute('aria-checked', 'mixed');
});

test('Switch: alterna y anuncia su estado', async () => {
    const screen = await render(Switch, { label: 'Despliegue automático' });
    const toggle = screen.getByRole('switch', { name: 'Despliegue automático' });
    await expect.element(toggle).not.toBeChecked();
    await toggle.click();
    await expect.element(toggle).toBeChecked();
});
