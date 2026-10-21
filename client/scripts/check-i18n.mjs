#!/usr/bin/env node
// Hace cumplir client/docs/i18n.md. Uso: pnpm check:i18n (I18N_STRICT=1 convierte en error
// las traducciones incompletas de los idiomas disponibles y de todos los demás).
//
// Errores:
// 1. Texto visible escrito a mano en componentes (.svelte): nodos de texto y atributos
//    `placeholder`, `title`, `alt`, `aria-label`, `label`…
// 2. Idiomas registrados en locales.ts sin archivo en messages/, o al revés.
// 3. Claves de una traducción que no existen en es.ts, o con otra forma (texto vs plural).
// 4. Traducciones cuyos `{parámetros}` no coinciden con los de es.ts.
// 5. Un idioma con `available: true` incompleto.
// Avisos: claves sin traducir de los idiomas en preparación.

import { readdirSync, readFileSync } from 'node:fs';
import { basename, extname, join, relative } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const CLIENT = fileURLToPath(new URL('..', import.meta.url));
const SRC = join(CLIENT, 'src');
const I18N = join(SRC, 'lib/i18n');
const MESSAGES = join(I18N, 'messages');
const REGISTRY = join(I18N, 'locales.ts');
const BASE = 'es';
const STRICT = process.env.I18N_STRICT === '1';

const errors = [];
const warnings = [];
const at = (file, line) => `${relative(CLIENT, file)}${line ? `:${line}` : ''}`;

// --- Diccionarios ---
// Los archivos de mensajes solo tienen `import type`, que Node elimina al cargar TypeScript.
async function loadMessages(code) {
    return (await import(pathToFileURL(join(MESSAGES, `${code}.ts`)).href)).default;
}

const isPlural = (node) => typeof node === 'object' && node !== null && typeof node.other === 'string';

/** Hojas del árbol: clave con puntos → texto o plural. */
function flatten(tree, prefix = '', out = new Map()) {
    for (const [key, node] of Object.entries(tree)) {
        const path = prefix ? `${prefix}.${key}` : key;
        if (typeof node === 'string' || isPlural(node)) out.set(path, node);
        else if (typeof node === 'object' && node !== null) flatten(node, path, out);
        else out.set(path, node);
    }
    return out;
}

const placeholders = (node) =>
    new Set(
        (typeof node === 'string' ? [node] : Object.values(node)).flatMap((text) =>
            [...String(text).matchAll(/\{(\w+)\}/g)].map((match) => match[1]),
        ),
    );
const sameSet = (a, b) => a.size === b.size && [...a].every((item) => b.has(item));

const registryText = readFileSync(REGISTRY, 'utf8');
const registered = [...registryText.matchAll(/\{\s*code:\s*['"]([\w-]+)['"][^}]*available:\s*(true|false)/g)].map(
    (match) => ({ code: match[1], available: match[2] === 'true' }),
);
const files = readdirSync(MESSAGES)
    .filter((name) => name.endsWith('.ts'))
    .map((name) => basename(name, '.ts'));

for (const { code } of registered) {
    if (!files.includes(code)) errors.push(`${at(REGISTRY)}: el idioma "${code}" no tiene messages/${code}.ts`);
    if (code !== BASE && !new RegExp(`\\b${code}:\\s*\\(\\)\\s*=>\\s*import\\(`).test(registryText)) {
        errors.push(`${at(REGISTRY)}: falta el cargador de "${code}" en LOADERS`);
    }
}
for (const code of files) {
    if (!registered.some((locale) => locale.code === code)) {
        errors.push(`${at(join(MESSAGES, `${code}.ts`))}: el idioma "${code}" no está registrado en LOCALES`);
    }
}

const base = flatten(await loadMessages(BASE));
for (const [key, node] of base) {
    if (typeof node !== 'string' && !isPlural(node))
        errors.push(`${at(join(MESSAGES, 'es.ts'))}: "${key}" no es texto ni plural`);
    if (isPlural(node) && !placeholders(node).has('count'))
        errors.push(`${at(join(MESSAGES, 'es.ts'))}: el plural "${key}" debe usar {count}`);
}

const summary = [];
for (const { code, available } of registered.filter((locale) => locale.code !== BASE && files.includes(locale.code))) {
    const file = join(MESSAGES, `${code}.ts`);
    const translated = flatten(await loadMessages(code));
    for (const [key, node] of translated) {
        const original = base.get(key);
        if (original === undefined) {
            errors.push(`${at(file)}: "${key}" no existe en es.ts`);
        } else if (isPlural(original) !== isPlural(node) || (typeof node !== 'string' && !isPlural(node))) {
            errors.push(
                `${at(file)}: "${key}" debe ser ${isPlural(original) ? 'un plural con `other`' : 'un texto'} como en es.ts`,
            );
        } else if (!sameSet(placeholders(original), placeholders(node))) {
            errors.push(
                `${at(file)}: "${key}" usa {${[...placeholders(node)].join(', ')}} y es.ts {${[...placeholders(original)].join(', ')}}`,
            );
        }
    }
    const missing = [...base.keys()].filter((key) => !translated.has(key));
    summary.push(`${code} ${base.size - missing.length}/${base.size}${available ? '' : ' (en preparación)'}`);
    if (missing.length > 0) {
        const message = `${at(file)}: faltan ${missing.length} de ${base.size} claves (${missing.slice(0, 5).join(', ')}${missing.length > 5 ? ', …' : ''})`;
        if (available || STRICT) errors.push(`${message}; un idioma disponible debe estar completo`);
        else warnings.push(message);
    }
}

// --- Texto escrito a mano en componentes ---
const VISIBLE_ATTRIBUTES =
    /\s(placeholder|title|alt|label|aria-label|aria-placeholder|aria-description|aria-roledescription|aria-valuetext)\s*=\s*"([^"]*)"/g;

/** Sustituye un tramo por espacios conservando los saltos de línea, para no mover las líneas. */
const blank = (text) => text.replace(/[^\n]/g, ' ');

function walk(dir) {
    return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
        const path = join(dir, entry.name);
        return entry.isDirectory() ? walk(path) : [path];
    });
}

// El grupo de rutas (dev) son herramientas internas que no existen en producción.
const DEV_ROUTES = join(SRC, 'routes', '(dev)');
for (const file of walk(SRC).filter((path) => extname(path) === '.svelte' && !path.startsWith(DEV_ROUTES))) {
    const source = readFileSync(file, 'utf8');
    let markup = source
        .replace(/<script[\s\S]*?<\/script>/g, blank)
        .replace(/<style[\s\S]*?<\/style>/g, blank)
        .replace(/<!--[\s\S]*?-->/g, blank)
        .replace(/&[a-z]+;|&#\d+;/gi, blank);
    let previous;
    do {
        previous = markup;
        markup = markup.replace(/\{[^{}]*\}/g, blank);
    } while (markup !== previous);
    const lineAt = (index) => source.slice(0, index).split('\n').length;

    for (const match of markup.matchAll(/>([^<>]+)</g)) {
        const text = match[1].trim();
        if (/\p{L}/u.test(text)) {
            errors.push(
                `${at(file, lineAt(match.index + match[0].indexOf(text)))}: texto sin traducir «${text}»: use t("…")`,
            );
        }
    }
    for (const match of markup.matchAll(VISIBLE_ATTRIBUTES)) {
        if (/\p{L}/u.test(match[2])) {
            errors.push(
                `${at(file, lineAt(match.index))}: ${match[1]}="${match[2]}" sin traducir: use ${match[1]}={t("…")}`,
            );
        }
    }
}

for (const warning of warnings) console.warn(`  aviso: ${warning}`);
if (errors.length > 0) {
    console.error(`check:i18n: ${errors.length} problema(s). Reglas: client/docs/i18n.md\n`);
    for (const error of errors) console.error(`  ${error}`);
    process.exit(1);
}
console.log(`check:i18n: OK (${base.size} claves; ${summary.join(', ')})`);
