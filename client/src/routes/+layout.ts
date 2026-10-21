// Tauri no tiene servidor Node: la app es una SPA servida por adapter-static con
// fallback a index.html. Ver https://svelte.dev/docs/kit/single-page-apps
export const ssr = false;

import { i18n } from '$lib/i18n';

/** El idioma guardado se carga antes del primer render, para no mostrar otro un instante. */
export async function load(): Promise<void> {
    await i18n.init();
}
