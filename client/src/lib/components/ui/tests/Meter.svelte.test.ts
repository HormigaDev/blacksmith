import { describe, expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import Meter from '../Meter.svelte';

describe('Meter', () => {
    test('proporción y estado, también con texto', async () => {
        await render(Meter, { label: 'Memoria', value: 50, max: 100, valueText: '50 MB / 100 MB' });
        const meter = page.getByRole('meter', { name: 'Memoria' });
        await expect.element(meter).toHaveAttribute('aria-valuenow', '50');
        await expect.element(meter).toHaveAttribute('aria-valuetext', '50 MB / 100 MB');
        await expect.element(page.getByText('50 %')).toBeVisible();
    });

    test('alto y crítico se dicen con palabras, no solo con color', async () => {
        await render(Meter, { label: 'Memoria', value: 80, max: 100 });
        await expect.element(page.getByText('· alto')).toBeVisible();
    });

    test('por encima del límite se queda en el 100 %', async () => {
        await render(Meter, { label: 'Memoria', value: 150, max: 100 });
        await expect.element(page.getByRole('meter', { name: 'Memoria' })).toHaveAttribute('aria-valuenow', '100');
        await expect.element(page.getByText('· crítico')).toBeVisible();
    });
});
