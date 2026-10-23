import { describe, expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import TabNav from '../TabNav.svelte';

describe('TabNav', () => {
    test('enlaces con nombre accesible y la pestaña actual marcada', async () => {
        await render(TabNav, {
            label: 'Secciones',
            items: [
                { href: '/a', label: 'Logs', current: true },
                { href: '/a/deployment', label: 'Despliegue', current: false },
            ],
        });
        await expect.element(page.getByRole('navigation', { name: 'Secciones' })).toBeVisible();
        await expect.element(page.getByRole('link', { name: 'Logs' })).toHaveAttribute('aria-current', 'page');
        await expect.element(page.getByRole('link', { name: 'Despliegue' })).not.toHaveAttribute('aria-current');
        await expect.element(page.getByRole('link', { name: 'Despliegue' })).toHaveAttribute('href', '/a/deployment');
    });
});
