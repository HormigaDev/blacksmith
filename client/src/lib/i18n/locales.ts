/**
 * Registro de idiomas. Para añadir uno (detalle en client/docs/i18n.md):
 *   1. Crear messages/<code>.ts exportando `{} satisfies Translation`.
 *   2. Registrarlo aquí y en LOADERS.
 *   3. Poner `available: true` solo cuando esté completo (`pnpm check:i18n`).
 */

import type { Translation } from './types';

export interface LocaleDefinition {
    readonly code: string;
    /** Nombre en su propio idioma (autónimo): no se traduce. */
    readonly name: string;
    /** Etiqueta BCP 47 para `Intl` y para `<html lang>`. */
    readonly intl: string;
    /** Solo los disponibles aparecen en el selector y en la detección del sistema. */
    readonly available: boolean;
}

export const LOCALES = [
    { code: 'es', name: 'Español', intl: 'es', available: true },
    { code: 'en', name: 'English', intl: 'en', available: false },
    { code: 'pt', name: 'Português', intl: 'pt-BR', available: false },
] as const satisfies readonly LocaleDefinition[];

export type LocaleCode = (typeof LOCALES)[number]['code'];
export type LocalePreference = LocaleCode | 'system';

export const BASE_LOCALE = 'es' satisfies LocaleCode;
export const DEFAULT_LOCALE_PREFERENCE: LocalePreference = 'system';
export const LOCALE_STORAGE_KEY = 'blacksmith:locale';

/** El idioma base se empaqueta siempre; los demás se cargan bajo demanda, en su propio chunk. */
export const LOADERS: Record<Exclude<LocaleCode, typeof BASE_LOCALE>, () => Promise<{ default: Translation }>> = {
    en: () => import('./messages/en'),
    pt: () => import('./messages/pt'),
};

const BY_CODE = new Map<string, LocaleDefinition>(LOCALES.map((locale) => [locale.code, locale]));

export function getLocale(code: LocaleCode): LocaleDefinition {
    return BY_CODE.get(code) ?? LOCALES[0];
}

export function availableLocales(): LocaleDefinition[] {
    return LOCALES.filter((locale) => locale.available);
}

export function isLocalePreference(value: unknown): value is LocalePreference {
    return value === 'system' || (typeof value === 'string' && BY_CODE.get(value)?.available === true);
}

/** Primer idioma del sistema que tengamos disponible, comparando por idioma (`pt-BR` → `pt`). */
export function detectSystemLocale(languages: readonly string[]): LocaleCode {
    for (const tag of languages) {
        const language = tag.split('-')[0]?.toLowerCase();
        const match = LOCALES.find((locale) => locale.available && locale.code === language);
        if (match) return match.code;
    }
    return BASE_LOCALE;
}
