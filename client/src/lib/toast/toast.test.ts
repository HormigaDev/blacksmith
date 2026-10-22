import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { toast } from './toast.svelte';

describe('toast', () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => {
        for (const item of toast.items) toast.dismiss(item.id);
        vi.useRealTimers();
    });

    test('se cierra sola tras su duración; los errores duran más', () => {
        toast.success('hecho');
        toast.error('falló');
        expect(toast.items.map((item) => item.variant)).toEqual(['success', 'danger']);
        vi.advanceTimersByTime(4_000);
        expect(toast.items.map((item) => item.variant)).toEqual(['danger']);
        vi.advanceTimersByTime(5_000);
        expect(toast.items).toHaveLength(0);
    });

    test('duration 0 la deja fija hasta cerrarla', () => {
        const id = toast.info('fija', { duration: 0 });
        vi.advanceTimersByTime(60_000);
        expect(toast.items).toHaveLength(1);
        toast.dismiss(id);
        expect(toast.items).toHaveLength(0);
    });

    test('nunca muestra más de 5: descarta las más antiguas', () => {
        for (let index = 0; index < 7; index++) toast.info(`n${index}`);
        expect(toast.items.map((item) => item.title)).toEqual(['n2', 'n3', 'n4', 'n5', 'n6']);
    });
});
