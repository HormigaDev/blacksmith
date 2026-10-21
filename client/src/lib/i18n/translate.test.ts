import assert from 'node:assert/strict';
import { test } from 'node:test';

import { translate } from './translate.ts';
import type { Translation } from './types.ts';

const base = {
    hello: 'Hola, {name}',
    servers: { one: '{count} servidor', other: '{count} servidores' },
    nested: { only: 'Solo en base' },
};

const run = (messages: object, locale: string, key: string, params?: Record<string, string | number>) =>
    // Árboles de prueba, ajenos a los mensajes reales de la app.
    translate(
        messages as Translation,
        base as Translation,
        key,
        params,
        new Intl.PluralRules(locale),
        new Intl.NumberFormat(locale),
    );

test('interpola parámetros', () => {
    assert.equal(run(base, 'es', 'hello', { name: 'Ana' }), 'Hola, Ana');
});

test('elige la forma plural y formatea el número según el idioma', () => {
    assert.equal(run(base, 'es', 'servers', { count: 1 }), '1 servidor');
    assert.equal(run(base, 'es', 'servers', { count: 12000 }), '12.000 servidores');
    const en = { servers: { one: '{count} server', other: '{count} servers' } };
    assert.equal(run(en, 'en', 'servers', { count: 12000 }), '12,000 servers');
});

test('usa el idioma base cuando falta la traducción o tiene otra forma', () => {
    assert.equal(run({}, 'en', 'nested.only'), 'Solo en base');
    assert.equal(run({ servers: 'texto en lugar de plural' }, 'en', 'servers', { count: 2 }), '2 servidores');
});

test('una clave inexistente se muestra tal cual, para que el fallo sea visible', () => {
    assert.equal(run(base, 'es', 'no.existe'), 'no.existe');
});
