import process from 'node:process';

import { sveltekit } from '@sveltejs/kit/vite';
import { playwright } from '@vitest/browser-playwright';
import { defineConfig } from 'vitest/config';

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig({
    plugins: [sveltekit()],

    // Se preoptimizan al arrancar: si Vite las descubre a mitad de una sesión, recarga la página
    // (en el Workbench) o rompe los imports en curso (primera ejecución de los tests de componente).
    optimizeDeps: {
        include: [
            '@fortawesome/free-brands-svg-icons',
            '@fortawesome/free-regular-svg-icons',
            '@fortawesome/free-solid-svg-icons',
            '@internationalized/date',
            '@tauri-apps/api/core',
            '@tauri-apps/api/mocks',
            '@tauri-apps/api/webview',
            '@tauri-apps/api/window',
            '@tauri-apps/plugin-dialog',
            'bits-ui',
        ],
    },

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent Vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
        port: 1420,
        strictPort: true,
        host: host || '127.0.0.1',
        hmr: host
            ? {
                  protocol: 'ws',
                  host,
                  port: 1421,
              }
            : undefined,
        watch: {
            // 3. tell Vite to ignore watching `src-tauri` (and the production build output: a
            //    `pnpm build` while the dev server runs must not reload the open pages)
            ignored: ['**/src-tauri/**', '**/build/**'],
        },
    },

    // Tests (client/docs/testing.md). Los E2E van aparte, con Playwright (playwright.config.ts).
    test: {
        // Un test sin aserciones es un test que no prueba nada.
        expect: { requireAssertions: true },
        projects: [
            {
                extends: true,
                test: {
                    name: 'unit',
                    environment: 'node',
                    include: ['src/**/*.test.ts'],
                    exclude: ['src/**/*.svelte.test.ts'],
                },
            },
            {
                extends: true,
                test: {
                    name: 'component',
                    include: ['src/**/*.svelte.test.ts'],
                    setupFiles: ['src/tests/setup.browser.ts'],
                    // Navegador real: bits-ui depende de eventos de puntero y de posicionamiento
                    // que jsdom no simula bien.
                    browser: {
                        enabled: true,
                        headless: true,
                        provider: playwright(),
                        instances: [{ browser: 'chromium' }],
                        screenshotFailures: false,
                    },
                },
            },
        ],
    },
});
