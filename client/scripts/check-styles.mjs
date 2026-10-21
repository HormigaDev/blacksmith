#!/usr/bin/env node
// Hace cumplir client/docs/styles.md. Uso: pnpm check:styles
//
// 1. Ningún color literal fuera de src/lib/styles/themes/.
// 2. Las propiedades de identidad (tipografía, espaciado, radios, sombras, capas, duraciones)
//    solo admiten tokens fuera de src/lib/styles/tokens.css y themes/.
// 3. Todo `var(--x)` usado existe (en tokens.css, en el tema de referencia o en el mismo archivo).
// 4. Todos los temas declaran exactamente las variables del tema de referencia (light.css),
//    están registrados en themes.ts e importados en global.css.

import { readdirSync, readFileSync } from 'node:fs';
import { basename, extname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const CLIENT = fileURLToPath(new URL('..', import.meta.url));
const SRC = join(CLIENT, 'src');
const STYLES = join(SRC, 'lib/styles');
const THEMES_DIR = join(STYLES, 'themes');
const TOKENS_FILE = join(STYLES, 'tokens.css');
const REFERENCE_THEME = join(THEMES_DIR, 'light.css');
const REGISTRY = join(SRC, 'lib/theme/themes.ts');
const GLOBAL_CSS = join(STYLES, 'global.css');

const errors = [];
const report = (file, line, message) => errors.push(`${relative(CLIENT, file)}:${line}: ${message}`);

const COLOR_LITERAL = /#[0-9a-f]{3,8}\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/i;
const NAMED_COLOR =
    /(?:^|[\s,(])(?:white|black|red|green|blue|yellow|orange|purple|pink|gray|grey|silver|maroon|navy|teal|olive|lime|aqua|fuchsia|brown|gold|indigo|violet|cyan|magenta|beige|tomato|crimson|coral|salmon)(?=$|[\s,)])/i;
const COLOR_PROPERTY =
    /^(?:color|background(?:-color)?|border(?:-(?:top|right|bottom|left|block|inline)(?:-(?:start|end))?)?(?:-color)?|outline(?:-color)?|fill|stroke|caret-color|accent-color|text-decoration(?:-color)?|column-rule(?:-color)?|scrollbar-color|box-shadow|text-shadow)$/;
const TOKEN_ONLY_PROPERTY =
    /^(?:font-family|font-size|font-weight|line-height|font|z-index|box-shadow|text-shadow|border-radius|border-(?:top|bottom)-(?:left|right)-radius|border-(?:start|end)-(?:start|end)-radius|(?:padding|margin)(?:-(?:top|right|bottom|left|block|inline)(?:-(?:start|end))?)?|gap|row-gap|column-gap|inset|top|right|bottom|left)$/;
const TIMED_PROPERTY =
    /^(?:transition|transition-duration|transition-delay|animation|animation-duration|animation-delay)$/;
const KEYWORDS = new Set(['0', 'auto', 'none', 'inherit', 'initial', 'unset', 'revert', 'normal', '100%', '50%']);

/** Un valor es válido si, quitando tokens y multiplicadores de calc(), solo quedan palabras clave. */
function isTokenized(value) {
    let rest = value.replace(/!important/i, '');
    let previous;
    do {
        previous = rest;
        rest = rest.replace(/var\(\s*--[\w-]+\s*(?:,[^()]*)?\)/g, ' § ');
    } while (rest !== previous);
    do {
        previous = rest;
        rest = rest.replace(/\b(?:calc|min|max|clamp)\(([^()]*)\)/g, (_, inner) =>
            inner.replace(/(?<![\w.%])-?\d*\.?\d+(?![\w.%])/g, ' ').replace(/[*/+-]/g, ' '),
        );
    } while (rest !== previous);
    return rest
        .split(/[\s,]+/)
        .filter(Boolean)
        .every((part) => part === '§' || KEYWORDS.has(part));
}

function walk(dir) {
    return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
        const path = join(dir, entry.name);
        return entry.isDirectory() ? walk(path) : [path];
    });
}

const lineAt = (text, index) => text.slice(0, index).split('\n').length;
const stripComments = (css) => css.replace(/\/\*[\s\S]*?\*\//g, (comment) => comment.replace(/[^\n]/g, ' '));

/** Bloques CSS de un archivo con su desplazamiento, para informar la línea real. */
function cssBlocks(file, text) {
    if (extname(file) === '.css') return [{ css: stripComments(text), offset: 0 }];
    const blocks = [];
    for (const match of text.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)) {
        blocks.push({ css: stripComments(match[1]), offset: match.index + match[0].indexOf(match[1]) });
    }
    // Atributos style="…" del marcado
    for (const match of text.matchAll(/\sstyle="([^"]*)"/g)) {
        blocks.push({ css: `${match[1]};`, offset: match.index });
    }
    return blocks;
}

function declarations(css) {
    return [...css.matchAll(/(--[\w-]+|[a-z-]+)\s*:\s*([^;{}]+?)\s*(?=;|})/g)].map((match) => ({
        property: match[1],
        value: match[2],
        index: match.index,
    }));
}

function declaredVariables(file) {
    const text = stripComments(readFileSync(file, 'utf8'));
    return new Set([...text.matchAll(/(--[\w-]+)\s*:/g)].map((match) => match[1]));
}

// --- 4. Coherencia de temas ---
const reference = declaredVariables(REFERENCE_THEME);
const themeFiles = readdirSync(THEMES_DIR).filter((name) => name.endsWith('.css'));
const registryIds = [...readFileSync(REGISTRY, 'utf8').matchAll(/\bid:\s*['"]([\w-]+)['"]/g)].map((m) => m[1]);
const globalCss = readFileSync(GLOBAL_CSS, 'utf8');

for (const name of themeFiles) {
    const file = join(THEMES_DIR, name);
    const id = basename(name, '.css');
    const vars = declaredVariables(file);
    for (const missing of [...reference].filter((v) => !vars.has(v)))
        report(file, 1, `falta ${missing} (definida en light.css)`);
    for (const extra of [...vars].filter((v) => !reference.has(v)))
        report(file, 1, `${extra} no existe en light.css: añádala primero al tema de referencia`);
    if (!new RegExp(`\\[data-theme=['"]${id}['"]\\]`).test(readFileSync(file, 'utf8')))
        report(file, 1, `debe usar el selector :root[data-theme="${id}"]`);
    if (!registryIds.includes(id)) report(REGISTRY, 1, `el tema "${id}" no está registrado en THEMES`);
    if (!globalCss.includes(`./themes/${name}`)) report(GLOBAL_CSS, 1, `falta @import "./themes/${name}"`);
}
for (const id of registryIds) {
    if (!themeFiles.includes(`${id}.css`))
        report(REGISTRY, 1, `el tema "${id}" no tiene src/lib/styles/themes/${id}.css`);
}

// --- 1, 2 y 3. Uso de tokens ---
const globalVariables = new Set([...declaredVariables(TOKENS_FILE), ...reference]);

for (const file of walk(SRC).filter((f) => ['.css', '.svelte'].includes(extname(f)))) {
    const isTheme = file.startsWith(THEMES_DIR);
    const isTokens = file === TOKENS_FILE;
    const text = readFileSync(file, 'utf8');
    const localVariables = declaredVariables(file);

    for (const { css, offset } of cssBlocks(file, text)) {
        for (const { property, value, index } of declarations(css)) {
            const line = lineAt(text, offset + index);
            const where = `${property}: ${value}`;
            if (!isTheme && (COLOR_LITERAL.test(value) || (COLOR_PROPERTY.test(property) && NAMED_COLOR.test(value)))) {
                report(file, line, `color literal en «${where}»: use un token --color-*`);
            }
            if (!isTheme && !isTokens && !property.startsWith('--')) {
                if (TOKEN_ONLY_PROPERTY.test(property) && !isTokenized(value)) {
                    report(
                        file,
                        line,
                        `valor literal en «${where}»: use un token (--space-*, --font-*, --radius-*, --shadow-*, --z-*…)`,
                    );
                }
                if (
                    TIMED_PROPERTY.test(property) &&
                    /(?<![\w-])\d*\.?\d+m?s\b/.test(value.replace(/var\([^)]*\)/g, ''))
                ) {
                    report(file, line, `duración literal en «${where}»: use --duration-*`);
                }
            }
        }
        for (const match of css.matchAll(/var\(\s*(--[\w-]+)/g)) {
            const name = match[1];
            // `--bits-*` las define bits-ui en tiempo de ejecución (medidas del ancla de un popover);
            // `--_*` son privadas del componente y pueden venir de una directiva `style:--_x={…}`.
            if (
                !globalVariables.has(name) &&
                !localVariables.has(name) &&
                !name.startsWith('--bits-') &&
                !name.startsWith('--_')
            ) {
                report(
                    file,
                    lineAt(text, offset + match.index),
                    `${name} no está definida en tokens.css ni en light.css`,
                );
            }
        }
    }
}

if (errors.length > 0) {
    console.error(`check:styles: ${errors.length} problema(s). Reglas: client/docs/styles.md\n`);
    for (const error of errors) console.error(`  ${error}`);
    process.exit(1);
}
console.log(`check:styles: OK (${themeFiles.length} temas, ${reference.size} tokens de tema)`);
