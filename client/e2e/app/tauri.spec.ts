import { expect, test, waitForApp } from '../support/console';
import { agentJson, mockTauri, tauriCalls } from '../support/tauri';

test.describe('Integración con Tauri (runtime simulado)', () => {
    test('sincroniza el tema con la ventana nativa', async ({ page }) => {
        await mockTauri(page);
        await page.emulateMedia({ colorScheme: 'light' });
        await page.goto('/');
        await waitForApp(page);

        await page.getByRole('button', { name: 'Tema' }).click();
        await page.getByRole('option', { name: 'Oscuro' }).click();

        await expect
            .poll(
                async () =>
                    (await tauriCalls(page)).filter((call) => call.cmd === 'plugin:window|set_theme').at(-1)?.args,
            )
            .toMatchObject({ label: 'main', value: 'dark' });
    });

    // El marco es fijo: aunque la página sea más alta que la ventana, el documento no se desplaza
    // y la barra de título (con los controles de ventana) sigue a la vista. Solo hace scroll el
    // contenido. Regla en client/docs/styles.md, «Marco fijo».
    test('el marco queda fijo: solo el contenido hace scroll', async ({ page }) => {
        const LOCAL = { id: 'local:4040', kind: 'local', host: 'localhost', port: 4040, username: '' };
        await page.setViewportSize({ width: 1024, height: 480 });
        await mockTauri(page, {
            servers_list: [LOCAL],
            local_agent_status: { name: 'blacksmith-agent', version: '1.0.0', status: 'ready' },
            agent_request: {
                byPath: {
                    'GET /api/v1/services': agentJson([
                        {
                            name: 'api',
                            displayName: 'API',
                            description: 'Pagos',
                            unit: 'bbel@api.service',
                            createdAt: 0,
                        },
                    ]),
                    'GET /api/v1/services/api/status': agentJson({
                        name: 'api',
                        unit: 'bbel@api.service',
                        loadState: 'loaded',
                        activeState: 'inactive',
                        subState: 'dead',
                        pid: null,
                        metrics: null,
                        deployed: false,
                        release: null,
                        previousRelease: null,
                    }),
                    'GET /api/v1/services/api/deployment': agentJson({
                        name: 'api',
                        repository: { owner: 'acme', name: 'api', branch: 'main' },
                        workingDirectory: '/srv/bbel/api',
                        access: 'account',
                        setupCommands: ['npm ci', 'npm run build', 'npm test'],
                        startCommand: 'npm start',
                        publishDir: null,
                        updatedAt: 1,
                    }),
                    'GET /api/v1/github': agentJson(null),
                    'GET /api/v1/services/api/site': agentJson(null),
                    'GET /api/v1/services/api/env': agentJson({
                        name: 'api',
                        unit: 'bbel@api.service',
                        file: '/home/ana/.config/bbel/env/services/api.env',
                        loadedByUnit: true,
                        globalLoadedByUnit: true,
                        variables: [],
                    }),
                    'GET /api/v1/services/api/deploys': agentJson([]),
                },
            },
            logs_follow: 1,
        });
        await page.goto('/');
        await waitForApp(page);
        await page.getByRole('button', { name: 'Conectar' }).click();
        await page.getByRole('main').getByRole('link', { name: 'localhost:4040' }).click();
        await page.getByRole('link', { name: 'API' }).first().click();
        await page.getByRole('link', { name: 'Despliegue' }).click();
        await expect(page.getByRole('button', { name: 'Desplegar' })).toBeAttached();

        const controls = page.getByRole('group', { name: 'Controles de la ventana' });
        const top = async () => (await controls.boundingBox())?.y;
        const before = await top();
        const content = page.getByRole('main');

        await content.hover();
        await page.mouse.wheel(0, 5000);
        await expect.poll(() => content.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
        // Ni el scroll programático (foco en un control al fondo) mueve el documento.
        await page.getByRole('button', { name: 'Desplegar' }).focus();
        await page.evaluate(() => window.scrollTo(0, 10_000));

        const documentScroll = await page.evaluate(() => ({
            scrollY: window.scrollY,
            overflow: document.documentElement.scrollHeight - window.innerHeight,
        }));
        expect(documentScroll).toEqual({ scrollY: 0, overflow: 0 });
        expect(await top()).toBe(before);
        await expect(controls).toBeInViewport({ ratio: 1 });
    });
});
