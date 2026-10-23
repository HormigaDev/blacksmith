/**
 * Tests E2E con Playwright (client/docs/testing.md).
 *
 * Proyectos:
 * - app-chromium / app-webkit: la app compilada (`vite preview`). Chromium ≈ WebView2 (Windows) y
 *   WebKit ≈ WebKitGTK (Linux): los dos motores reales de Tauri. `E2E_WEBKIT=0` omite WebKit.
 * - workbench: accesibilidad (axe) y humo de todas las historias, contra el servidor de desarrollo.
 * - visual: regresión visual de cada historia. Opt-in (`pnpm test:visual`): las capturas dependen
 *   de las fuentes del sistema, así que se generan y comparan siempre en la misma máquina o en CI.
 */
import process from 'node:process';

import { defineConfig, devices } from '@playwright/test';

const CI = Boolean(process.env.CI);

/**
 * Solo se arrancan los servidores de los proyectos pedidos (`--project`). Además, la app se sirve
 * desde un build hecho ANTES (ver `test:e2e` en package.json): compilar mientras corre el
 * servidor de desarrollo reescribe `.svelte-kit/generated`, y el Workbench se recarga en mitad
 * de los tests.
 */
const requested = process.argv.flatMap((arg, index, all) =>
    arg.startsWith('--project=') ? [arg.slice('--project='.length)] : arg === '--project' ? [all[index + 1] ?? ''] : [],
);
const runs = (prefix: string) => requested.length === 0 || requested.some((name) => name.startsWith(prefix));
const APP_URL = 'http://localhost:4173';
const DEV_URL = 'http://localhost:1420';

export default defineConfig({
    testDir: 'e2e',
    fullyParallel: true,
    // Cada worker es un navegador (~300-500 MB) y además corren el build y el servidor de desarrollo:
    // sin límite, en una máquina con el editor abierto se llegó a agotar la memoria (OOM killer).
    workers: Number(process.env.E2E_WORKERS ?? 2),
    forbidOnly: CI,
    retries: CI ? 2 : 0,
    reporter: CI ? [['github'], ['html', { open: 'never' }]] : [['list'], ['html', { open: 'never' }]],
    use: {
        trace: 'on-first-retry',
        screenshot: 'only-on-failure',
        locale: 'es-ES',
        timezoneId: 'America/Sao_Paulo',
    },
    expect: {
        // Estricto: un cambio de radio o de 1 px de borde toca muy pocos píxeles y debe detectarse.
        toHaveScreenshot: { maxDiffPixels: 0, animations: 'disabled', caret: 'hide' },
    },
    // Relativa a este archivo: una sola carpeta de capturas de referencia, versionada.
    snapshotPathTemplate: 'e2e/__screenshots__/{projectName}/{arg}{ext}',
    webServer: [
        ...(runs('app')
            ? [
                  {
                      command: 'pnpm preview --port 4173 --strictPort',
                      url: APP_URL,
                      reuseExistingServer: !CI,
                      timeout: 60_000,
                  },
              ]
            : []),
        ...(runs('workbench') || runs('visual')
            ? [
                  {
                      command: 'pnpm dev',
                      url: `${DEV_URL}/dev/workbench`,
                      reuseExistingServer: !CI,
                      timeout: 120_000,
                  },
              ]
            : []),
    ],
    projects: [
        {
            name: 'app-chromium',
            testDir: 'e2e/app',
            use: { ...devices['Desktop Chrome'], baseURL: APP_URL },
        },
        ...(process.env.E2E_WEBKIT === '0'
            ? []
            : [{ name: 'app-webkit', testDir: 'e2e/app', use: { ...devices['Desktop Safari'], baseURL: APP_URL } }]),
        {
            name: 'workbench',
            testDir: 'e2e/workbench',
            testIgnore: '**/*.visual.spec.ts',
            use: { ...devices['Desktop Chrome'], baseURL: DEV_URL },
        },
        {
            name: 'visual',
            testDir: 'e2e/workbench',
            testMatch: '**/*.visual.spec.ts',
            use: {
                ...devices['Desktop Chrome'],
                baseURL: DEV_URL,
                // Alto de sobra: una historia más alta que la ventana saldría recortada en su captura.
                viewport: { width: 1280, height: 1600 },
                deviceScaleFactor: 1,
                // Rasterizado independiente de la configuración de la máquina (fontconfig, GPU,
                // perfil de color). Junto con e2e/support/visual.ts hace la captura reproducible.
                launchOptions: {
                    args: [
                        '--font-render-hinting=none',
                        '--disable-font-subpixel-positioning',
                        '--disable-lcd-text',
                        '--force-color-profile=srgb',
                        '--disable-gpu',
                    ],
                },
            },
        },
    ],
});
