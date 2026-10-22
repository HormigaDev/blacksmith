import { expect, test } from 'vitest';

import { translate } from './translate';
import type { Translation } from './types';

const base = {
    hello: 'Hola, {name}',
    servers: { one: '{count} servidor', other: '{count} servidores' },
    nested: { only: 'Solo en base' },
};

// Árboles de prueba, ajenos a los mensajes reales de la app.
const run = (messages: object, locale: string, key: string, params?: Record<string, string | number>) =>
    translate(
        messages as Translation,
        base as Translation,
        key,
        params,
        new Intl.PluralRules(locale),
        new Intl.NumberFormat(locale),
    );

test('interpola parámetros', () => {
    expect(run(base, 'es', 'hello', { name: 'Ana' })).toBe('Hola, Ana');
});

test('elige la forma plural y formatea el número según el idioma', () => {
    expect(run(base, 'es', 'servers', { count: 1 })).toBe('1 servidor');
    expect(run(base, 'es', 'servers', { count: 12000 })).toBe('12.000 servidores');
    const en = { servers: { one: '{count} server', other: '{count} servers' } };
    expect(run(en, 'en', 'servers', { count: 12000 })).toBe('12,000 servers');
});

test('usa el idioma base cuando falta la traducción o tiene otra forma', () => {
    expect(run({}, 'en', 'nested.only')).toBe('Solo en base');
    expect(run({ servers: 'texto en lugar de plural' }, 'en', 'servers', { count: 2 })).toBe('2 servidores');
});

test('una clave inexistente se muestra tal cual, para que el fallo sea visible', () => {
    expect(run(base, 'es', 'no.existe')).toBe('no.existe');
});
