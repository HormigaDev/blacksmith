import { describe, expect, test } from 'vitest';

import { formatDuration } from './format';

describe('formatDuration', () => {
    test('las dos unidades más grandes', () => {
        // El formato exacto depende del idioma: se comprueban las cifras y el orden.
        expect(formatDuration(42)).toMatch(/^42\s?\S+$/);
        expect(formatDuration(2 * 3600 + 5 * 60 + 9)).toMatch(/^2\s?\S+ 5\s?\S+$/);
        expect(formatDuration(3 * 86400 + 4 * 3600 + 59)).toMatch(/^3\s?\S+ 4\s?\S+$/);
        expect(formatDuration(3600)).toMatch(/^1\s?\S+$/);
    });

    test('nunca negativo ni vacío', () => {
        expect(formatDuration(-5)).toMatch(/^0\s?\S+$/);
        expect(formatDuration(0.4)).toMatch(/^0\s?\S+$/);
    });
});
