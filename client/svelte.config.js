// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
    preprocess: vitePreprocess(),
    kit: {
        adapter: adapter({
            fallback: 'index.html',
        }),
        typescript: {
            // Un solo proyecto de TypeScript: el editor, svelte-check y `pnpm check` también cubren
            // los tests E2E y la configuración de Playwright, que viven fuera de src/.
            config: (tsconfig) => {
                tsconfig.include.push('../playwright.config.ts', '../e2e/**/*.ts');
                return tsconfig;
            },
        },
    },
};

export default config;
