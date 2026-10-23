import { describe, expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import StatTile from '../StatTile.svelte';

describe('StatTile', () => {
    test('etiqueta, valor y detalle, como región con nombre', async () => {
        await render(StatTile, { label: 'Memoria', value: '512 MB', detail: 'Pico 640 MB' });
        const tile = page.getByRole('region', { name: 'Memoria' });
        await expect.element(tile.getByText('512 MB')).toBeVisible();
        await expect.element(tile.getByText('Pico 640 MB')).toBeVisible();
    });
});
