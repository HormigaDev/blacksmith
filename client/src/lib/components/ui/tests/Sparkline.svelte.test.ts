import { describe, expect, test } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import Sparkline from '../Sparkline.svelte';

const format = (value: number) => `${value} %`;

describe('Sparkline', () => {
    test('resume la serie en su descripción y anuncia la última lectura', async () => {
        await render(Sparkline, { label: 'CPU', values: [10, 30, null, 20], format });
        const chart = page.getByRole('slider', { name: 'CPU' });
        await expect.element(chart).toHaveAccessibleDescription('CPU: ahora 20 %; mínimo 10 %, máximo 30 %.');
        await expect.element(chart).toHaveAttribute('aria-valuetext', '20 %');
    });

    test('con el teclado recorre las lecturas; un hueco se dice como tal', async () => {
        await render(Sparkline, { label: 'CPU', values: [10, null, 20], format });
        const chart = page.getByRole('slider', { name: 'CPU' });
        await chart.element().focus();
        await userEvent.keyboard('{ArrowLeft}');
        await expect.element(chart).toHaveAttribute('aria-valuetext', 'Sin datos');
        await userEvent.keyboard('{Home}');
        await expect.element(chart).toHaveAttribute('aria-valuetext', '10 %');
        await expect.element(page.getByText('10 %', { exact: true })).toBeVisible();
        await userEvent.keyboard('{Escape}');
        await expect.element(page.getByText('10 %', { exact: true })).not.toBeInTheDocument();
    });

    test('sin datos', async () => {
        await render(Sparkline, { label: 'CPU', values: [], format });
        await expect
            .element(page.getByRole('slider', { name: 'CPU' }))
            .toHaveAccessibleDescription('CPU: sin datos todavía.');
    });
});
