import { createRawSnippet } from 'svelte';
import { describe, expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import Panel from '../Panel.svelte';

const text = (content: string) => createRawSnippet(() => ({ render: () => `<p>${content}</p>` }));

describe('Panel', () => {
    test('con título: región con nombre accesible, acciones y contenido', async () => {
        await render(Panel, {
            title: 'Despliegues',
            actions: createRawSnippet(() => ({ render: () => '<button type="button">Desplegar</button>' })),
            children: text('Contenido'),
        });
        await expect.element(page.getByRole('region', { name: 'Despliegues' })).toBeVisible();
        await expect.element(page.getByRole('button', { name: 'Desplegar' })).toBeVisible();
        await expect.element(page.getByText('Contenido')).toBeVisible();
    });

    test('sin título no hay cabecera', async () => {
        await render(Panel, { children: text('Solo contenido') });
        await expect.element(page.getByRole('heading')).not.toBeInTheDocument();
    });
});
