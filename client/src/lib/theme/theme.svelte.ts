import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

import {
    type ColorScheme,
    DEFAULT_PREFERENCE,
    getTheme,
    isThemePreference,
    SYSTEM_THEMES,
    THEME_STORAGE_KEY,
    type ThemeDefinition,
    type ThemePreference,
} from './themes';

const DARK_QUERY = '(prefers-color-scheme: dark)';

function systemScheme(): ColorScheme {
    return window.matchMedia(DARK_QUERY).matches ? 'dark' : 'light';
}

function readPreference(): ThemePreference {
    try {
        const stored = localStorage.getItem(THEME_STORAGE_KEY);
        return isThemePreference(stored) ? stored : DEFAULT_PREFERENCE;
    } catch {
        return DEFAULT_PREFERENCE;
    }
}

function writePreference(preference: ThemePreference): void {
    try {
        localStorage.setItem(THEME_STORAGE_KEY, preference);
    } catch {
        // Sin almacenamiento el tema sigue funcionando; solo no se recuerda.
    }
}

/** Estado global del tema. Única fuente de verdad: los componentes leen `theme.current`. */
class ThemeState {
    #preference = $state<ThemePreference>(readPreference());
    #systemScheme = $state<ColorScheme>(systemScheme());

    readonly current: ThemeDefinition = $derived(
        getTheme(this.#preference === 'system' ? SYSTEM_THEMES[this.#systemScheme] : this.#preference),
    );

    get preference(): ThemePreference {
        return this.#preference;
    }

    set preference(preference: ThemePreference) {
        this.#preference = preference;
        writePreference(preference);
    }

    /** Sigue los cambios de tema del sistema operativo. Devuelve la función de limpieza. */
    followSystem(): () => void {
        const media = window.matchMedia(DARK_QUERY);
        const update = () => {
            this.#systemScheme = media.matches ? 'dark' : 'light';
        };
        media.addEventListener('change', update);
        return () => media.removeEventListener('change', update);
    }
}

export const theme = new ThemeState();

/** Aplica el tema al documento y a la ventana nativa (barra de título). */
export function applyTheme({ id, scheme }: ThemeDefinition): void {
    document.documentElement.dataset.theme = id;
    if (isTauri()) {
        getCurrentWindow()
            .setTheme(scheme)
            .catch((error: unknown) => console.warn('No se pudo aplicar el tema a la ventana', error));
    }
}
