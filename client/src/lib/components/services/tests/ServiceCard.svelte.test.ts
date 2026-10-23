import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import ServiceCard from '../ServiceCard.svelte';

const SERVICE = {
    name: 'api',
    displayName: 'API de pagos',
    description: 'Cobros con tarjeta.',
    unit: 'blacksmith-api.service',
    createdAt: 0,
};

describe('ServiceCard', () => {
    test('muestra nombre, unidad y descripción, y enlaza a su página', async () => {
        await render(ServiceCard, { service: SERVICE, href: '/servers/x/services/api', onRemove: vi.fn() });
        await expect
            .element(page.getByRole('link', { name: 'API de pagos' }))
            .toHaveAttribute('href', '/servers/x/services/api');
        await expect.element(page.getByText('blacksmith-api.service')).toBeVisible();
        await expect.element(page.getByText('Cobros con tarjeta.')).toBeVisible();
    });

    test('quitar del registro exige confirmación', async () => {
        const onRemove = vi.fn();
        await render(ServiceCard, { service: SERVICE, onRemove });
        await page.getByRole('button', { name: 'Quitar API de pagos' }).click();
        await expect.element(page.getByText(/También se borrarán sus variables de entorno/)).toBeVisible();
        expect(onRemove).not.toHaveBeenCalled();
        await page.getByRole('button', { name: 'Quitar', exact: true }).click();
        await expect.poll(() => onRemove.mock.calls.length).toBe(1);
    });

    test('sin descripción no deja un párrafo vacío', async () => {
        await render(ServiceCard, { service: { ...SERVICE, description: '' }, onRemove: vi.fn() });
        await expect.element(page.getByRole('link')).not.toBeInTheDocument();
        expect(document.querySelector('.description')).toBeNull();
    });
});
