// Lógica pura de traducción, sin estado: búsqueda, plurales e interpolación.

import type { ParamValue, PluralMessage, Translation } from './types';

type Node = string | { readonly [key: string]: Node | undefined };

function lookup(tree: Translation, key: string): Node | undefined {
    let node: Node | undefined = tree as Node;
    for (const part of key.split('.')) {
        if (node === undefined || typeof node === 'string') return undefined;
        node = node[part];
    }
    return node;
}

function isPlural(node: Node | undefined): node is PluralMessage {
    return typeof node === 'object' && typeof node.other === 'string';
}

/**
 * Traduce `key` con `messages`; si falta, usa `fallback` (el idioma base). Si tampoco existe
 * (solo posible saltándose los tipos), devuelve la clave para que el fallo sea visible.
 */
export function translate(
    messages: Translation,
    fallback: Translation,
    key: string,
    params: Readonly<Record<string, ParamValue>> | undefined,
    plurals: Intl.PluralRules,
    numbers: Intl.NumberFormat,
): string {
    let node = lookup(messages, key);
    const base = lookup(fallback, key);
    if (node === undefined || typeof node !== typeof base) node = base;

    let template: string | undefined;
    if (typeof node === 'string') {
        template = node;
    } else if (isPlural(node)) {
        const count = Number(params?.count ?? 0);
        template = node[plurals.select(count)] ?? node.other;
    }
    if (template === undefined) return key;
    if (params === undefined) return template;

    return template.replace(/\{(\w+)\}/g, (placeholder, name: string) => {
        const value = params[name];
        if (value === undefined) return placeholder;
        return typeof value === 'number' ? numbers.format(value) : value;
    });
}
