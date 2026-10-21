/**
 * Registro de temas. Para añadir uno (detalle en client/docs/styles.md):
 *   1. Crear src/lib/styles/themes/<id>.css con `:root[data-theme="<id>"]` y TODAS las
 *      variables de light.css.
 *   2. Importarlo en src/lib/styles/global.css.
 *   3. Registrarlo aquí y añadir su nombre en `theme.names` de los mensajes (i18n).
 * `pnpm check:styles` falla si falta cualquiera de los tres pasos.
 */

export type ColorScheme = 'light' | 'dark';

export interface ThemeDefinition {
    /**
     * Valor de `data-theme` en <html> y nombre del archivo CSS. Su nombre visible es la clave
     * `theme.names.<id>` de los mensajes.
     */
    readonly id: string;
    /** Esquema nativo: controles del sistema y barra de título de la ventana. */
    readonly scheme: ColorScheme;
}

export const THEMES = [
    { id: 'light', scheme: 'light' },
    { id: 'dark', scheme: 'dark' },
] as const satisfies readonly ThemeDefinition[];

export type ThemeId = (typeof THEMES)[number]['id'];

/** `system` sigue la preferencia del sistema operativo. */
export type ThemePreference = ThemeId | 'system';

/** Tema que se usa para cada esquema cuando la preferencia es `system`. */
export const SYSTEM_THEMES: Readonly<Record<ColorScheme, ThemeId>> = {
    light: 'light',
    dark: 'dark',
};

export const DEFAULT_PREFERENCE: ThemePreference = 'system';

/** Clave de localStorage. El script de arranque de src/app.html usa la misma. */
export const THEME_STORAGE_KEY = 'blacksmith:theme';

const BY_ID = new Map<string, ThemeDefinition>(THEMES.map((theme) => [theme.id, theme]));

export function getTheme(id: ThemeId): ThemeDefinition {
    // El tipo garantiza que existe; el respaldo solo protege ante un registro inconsistente.
    return BY_ID.get(id) ?? THEMES[0];
}

export function isThemePreference(value: unknown): value is ThemePreference {
    return value === 'system' || (typeof value === 'string' && BY_ID.has(value));
}
