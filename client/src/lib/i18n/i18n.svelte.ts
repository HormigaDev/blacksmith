import {
    BASE_LOCALE,
    DEFAULT_LOCALE_PREFERENCE,
    detectSystemLocale,
    getLocale,
    isLocalePreference,
    LOADERS,
    LOCALE_STORAGE_KEY,
    type LocaleCode,
    type LocaleDefinition,
    type LocalePreference,
} from './locales';
import base from './messages/es';
import { translate } from './translate';
import type { MessageKey, TranslateArgs, Translation } from './types';

function readPreference(): LocalePreference {
    try {
        const stored = localStorage.getItem(LOCALE_STORAGE_KEY);
        return isLocalePreference(stored) ? stored : DEFAULT_LOCALE_PREFERENCE;
    } catch {
        return DEFAULT_LOCALE_PREFERENCE;
    }
}

function writePreference(preference: LocalePreference): void {
    try {
        localStorage.setItem(LOCALE_STORAGE_KEY, preference);
    } catch {
        // Sin almacenamiento el idioma sigue funcionando; solo no se recuerda.
    }
}

function resolve(preference: LocalePreference): LocaleCode {
    return preference === 'system' ? detectSystemLocale(navigator.languages) : preference;
}

/** Estado global del idioma. Única fuente de verdad: los componentes usan `t()`. */
class I18nState {
    #preference = $state<LocalePreference>(readPreference());
    #locale = $state<LocaleCode>(BASE_LOCALE);
    #messages = $state.raw<Translation>(base);
    /** Descarta cargas lentas que llegan después de un cambio de idioma más reciente. */
    #request = 0;

    readonly current: LocaleDefinition = $derived(getLocale(this.#locale));
    readonly #plurals = $derived(new Intl.PluralRules(this.current.intl));
    readonly #numbers = $derived(new Intl.NumberFormat(this.current.intl));

    get preference(): LocalePreference {
        return this.#preference;
    }

    get locale(): LocaleCode {
        return this.#locale;
    }

    /** Carga el idioma guardado. Se espera en src/routes/+layout.ts, antes del primer render. */
    init(): Promise<void> {
        return this.#activate(resolve(this.#preference));
    }

    async setPreference(preference: LocalePreference): Promise<void> {
        this.#preference = preference;
        writePreference(preference);
        await this.#activate(resolve(preference));
    }

    /** Función de traducción. Es reactiva: al cambiar el idioma se vuelve a evaluar sola. */
    readonly t = <K extends MessageKey>(key: K, ...[params]: TranslateArgs<K>): string =>
        translate(this.#messages, base, key, params, this.#plurals, this.#numbers);

    async #activate(code: LocaleCode): Promise<void> {
        const request = ++this.#request;
        let messages: Translation = base;
        if (code !== BASE_LOCALE) {
            try {
                messages = (await LOADERS[code]()).default;
            } catch (error) {
                console.error(`No se pudo cargar el idioma "${code}"; se mantiene el actual`, error);
                return;
            }
        }
        if (request !== this.#request) return;
        this.#locale = code;
        this.#messages = messages;
    }
}

export const i18n = new I18nState();
export const t = i18n.t;
