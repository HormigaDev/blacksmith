/**
 * Entorno determinista para la regresión visual: la misma captura, píxel a píxel, en cualquier
 * máquina Linux (client/docs/testing.md §3, "Visuales").
 *
 * Qué haría variar una captura y cómo se neutraliza:
 * - Fuentes del sistema (`system-ui`): se sustituyen por fuentes versionadas en
 *   e2e/fixtures/fonts (Inter y JetBrains Mono, OFL, con SHA256SUMS). Si algún carácter visible
 *   queda fuera de su cobertura, el navegador tiraría de una fuente del sistema: se comprueba y el
 *   test falla en vez de producir una captura no reproducible.
 * - Rasterizado (hinting, subpíxel, perfil de color): flags de Chromium en playwright.config.ts.
 * - Versión del navegador: la fija la versión exacta de @playwright/test (package.json).
 * - Fecha y hora (p. ej. el día de hoy del calendario): reloj congelado.
 * - Animaciones, cursor y movimiento: desactivados en la captura y con `reducedMotion`.
 * - Sistema operativo: Windows y macOS rasterizan el texto con otros motores; no es corregible, así
 *   que las capturas solo se generan y comparan en Linux (lo impone stories.visual.spec.ts).
 */
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, type Page } from '@playwright/test';

const FONTS_DIR = join(import.meta.dirname, '..', 'fixtures', 'fonts');
const FONT_ROUTE = '/__visual-fonts__/';
export const VISUAL_SANS = 'Blacksmith Visual Sans';
export const VISUAL_MONO = 'Blacksmith Visual Mono';
/** Fecha fija para todo lo que dependa de "ahora". */
export const FROZEN_NOW = new Date('2026-01-15T10:00:00-03:00');

// Rangos oficiales de Fontsource para los subconjuntos latin y latin-ext.
const LATIN =
    'U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD';
const LATIN_EXT =
    'U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF';

const FACES = [
    ...[400, 500, 600, 700].flatMap((weight) => [
        { family: VISUAL_SANS, weight, file: `inter-latin-${weight}-normal.woff2`, range: LATIN },
        { family: VISUAL_SANS, weight, file: `inter-latin-ext-${weight}-normal.woff2`, range: LATIN_EXT },
    ]),
    { family: VISUAL_MONO, weight: 400, file: 'jetbrains-mono-latin-400-normal.woff2', range: LATIN },
    { family: VISUAL_MONO, weight: 400, file: 'jetbrains-mono-latin-ext-400-normal.woff2', range: LATIN_EXT },
];

/** Las fuentes deben ser exactamente las versionadas: un glifo distinto invalida todas las capturas. */
function verifiedFonts(): Map<string, Buffer> {
    const expected = new Map(
        readFileSync(join(FONTS_DIR, 'SHA256SUMS'), 'utf8')
            .trim()
            .split('\n')
            .map((line) => {
                const [hash, file] = line.split(/\s+/);
                return [file, hash] as const;
            }),
    );
    const fonts = new Map<string, Buffer>();
    for (const { file } of FACES) {
        const bytes = readFileSync(join(FONTS_DIR, file));
        const hash = createHash('sha256').update(bytes).digest('hex');
        if (hash !== expected.get(file)) throw new Error(`${file} no coincide con e2e/fixtures/fonts/SHA256SUMS`);
        fonts.set(file, bytes);
    }
    return fonts;
}

const FONTS = verifiedFonts();

const STYLE = [
    ...FACES.map(
        ({ family, weight, file, range }) =>
            `@font-face { font-family: '${family}'; font-style: normal; font-weight: ${weight}; ` +
            `font-display: block; src: url('${FONT_ROUTE}${file}') format('woff2'); unicode-range: ${range}; }`,
    ),
    `:root { --font-sans: '${VISUAL_SANS}' !important; --font-mono: '${VISUAL_MONO}' !important; }`,
].join('\n');

/** Antes de navegar: sirve las fuentes versionadas y congela el reloj. */
export async function prepareVisualPage(page: Page): Promise<void> {
    await page.route(`**${FONT_ROUTE}*`, (route) => {
        const file = new URL(route.request().url()).pathname.slice(FONT_ROUTE.length);
        const body = FONTS.get(file);
        return body ? route.fulfill({ body, contentType: 'font/woff2' }) : route.abort();
    });
    await page.clock.setFixedTime(FROZEN_NOW);
}

/** Tras navegar: aplica las fuentes, espera a que carguen y verifica que nada usa otra fuente. */
export async function applyVisualFonts(page: Page): Promise<void> {
    await page.addStyleTag({ content: STYLE });
    // En desarrollo, Vite inyecta el CSS global por JavaScript tras montar la app: hasta que llega,
    // el body usa la fuente por defecto. Esperar a la fuente de test garantiza ambas cosas.
    await expect
        .poll(() => page.evaluate(() => getComputedStyle(document.body).fontFamily), {
            message: 'el body debe usar la fuente de test',
        })
        .toContain(VISUAL_SANS);
    const report = await page.evaluate(
        async ({ sans, mono }) => {
            const faces = [...document.fonts].filter(
                (face) => face.family.replace(/["']/g, '') === sans || face.family.replace(/["']/g, '') === mono,
            );
            await Promise.all(faces.map((face) => face.load()));
            await document.fonts.ready;
            return {
                faces: faces.length,
                failed: faces.filter((face) => face.status !== 'loaded').map((face) => `${face.family} ${face.weight}`),
                body: getComputedStyle(document.body).fontFamily,
            };
        },
        { sans: VISUAL_SANS, mono: VISUAL_MONO },
    );
    expect(report.faces, 'fuentes de test registradas').toBe(FACES.length);
    expect(report.failed, 'fuentes de test sin cargar').toEqual([]);
    expect(report.body).toContain(VISUAL_SANS);
}

/** Caracteres visibles que ninguna fuente versionada cubre (caerían en una fuente del sistema). */
export async function uncoveredCharacters(page: Page, selector: string): Promise<string[]> {
    const ranges = `${LATIN},${LATIN_EXT}`.split(',').map((range) => {
        const [start, end = start] = range.replace('U+', '').split('-');
        return [Number.parseInt(start ?? '0', 16), Number.parseInt(end ?? '0', 16)] as const;
    });
    const text = await page.locator(selector).evaluateAll((elements) =>
        elements
            .map((element) => (element as HTMLElement).innerText)
            .join('')
            .replace(/\s/g, ''),
    );
    const covered = (code: number) => ranges.some(([start, end]) => code >= start && code <= end);
    return [...new Set(text)].filter((char) => !covered(char.codePointAt(0) ?? 0));
}
