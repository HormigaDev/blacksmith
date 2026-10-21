/**
 * Mensajes en español: idioma BASE y fuente de verdad de las claves.
 * - Toda clave existe aquí primero; los demás idiomas solo traducen claves de este archivo.
 * - Parámetros con `{nombre}`. Plurales: objeto con `one` / `other` (y `zero`, `two`,
 *   `few`, `many` si el idioma los usa), siempre con `{count}`.
 * - Agrupar por pantalla o área funcional; `common` solo para lo verdaderamente compartido.
 * Reglas completas: client/docs/i18n.md
 */
export default {
    common: {
        system: 'Sistema',
    },
    theme: {
        label: 'Tema',
        names: {
            light: 'Claro',
            dark: 'Oscuro',
        },
    },
    locale: {
        label: 'Idioma',
    },
    dashboard: {
        title: 'Panel',
        empty: {
            title: 'Sin servidores todavía',
            description: 'Aquí aparecerán los servidores y servicios que administres.',
        },
    },
} as const;
