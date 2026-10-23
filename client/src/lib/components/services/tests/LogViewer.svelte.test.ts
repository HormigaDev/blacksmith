import { describe, expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { LogLine } from '$lib/services/log-session.svelte';

import LogViewer from '../LogViewer.svelte';

const line = (id: number, message: string, priority: number | null = 6): LogLine => ({
    id,
    timestampUs: Date.UTC(2026, 9, 22, 12, 0, 0, 123) * 1000,
    priority,
    pid: 42,
    identifier: 'api',
    message,
});

const log = () => page.getByRole('log', { name: 'Logs de API' });

describe('LogViewer', () => {
    test('muestra las entradas sin códigos de color y con su prioridad', async () => {
        await render(LogViewer, {
            name: 'API',
            status: 'live',
            lines: [line(1, 'arrancando'), line(2, '\u001b[31mfallo\u001b[0m grave', 3), line(3, 'cuidado', 4)],
        });
        await expect.element(page.getByText('En vivo')).toBeVisible();
        await expect.poll(() => log().element().textContent).toContain('arrancando');
        await expect.element(page.getByText('fallo grave')).toBeVisible();
        expect(log().element().textContent).not.toContain('\u001b');
        expect(page.getByText('fallo grave').element().closest('.line')?.classList.contains('error')).toBe(true);
        expect(page.getByText('cuidado').element().closest('.line')?.classList.contains('warning')).toBe(true);
    });

    test('en vivo y sin entradas lo dice', async () => {
        await render(LogViewer, { name: 'API', status: 'live', lines: [] });
        await expect.element(page.getByText('Aún no hay entradas.')).toBeVisible();
    });

    test('fin y error: motivo y botón para reconectar', async () => {
        const onReconnect = vi.fn();
        const view = await render(LogViewer, {
            name: 'API',
            status: 'ended',
            endReason: 'connectionLost',
            lines: [],
            onReconnect,
        });
        await expect.element(page.getByText('Se perdió la conexión con el agente.')).toBeVisible();
        await page.getByRole('button', { name: 'Reconectar' }).click();
        expect(onReconnect).toHaveBeenCalledOnce();

        await view.rerender({ status: 'error', error: 'Hay demasiados visores…', endReason: null });
        await expect.element(page.getByRole('alert')).toHaveTextContent('Hay demasiados visores…');
    });

    test('se queda al final mientras llegan entradas, salvo si el usuario subió', async () => {
        const many = Array.from({ length: 200 }, (_, i) => line(i, `línea ${i}`));
        const view = await render(LogViewer, { name: 'API', status: 'live', lines: many });
        const element = log().element() as HTMLElement;
        const atBottom = () => element.scrollHeight - element.scrollTop - element.clientHeight < 2;
        await expect.poll(atBottom).toBe(true);

        await view.rerender({ lines: [...many, line(200, 'nueva')] });
        await expect.poll(atBottom).toBe(true);

        element.scrollTop = 0;
        element.dispatchEvent(new Event('scroll'));
        await view.rerender({ lines: [...many, line(200, 'nueva'), line(201, 'otra')] });
        await new Promise((resolve) => setTimeout(resolve, 50));
        expect(element.scrollTop).toBe(0);
    });
});
