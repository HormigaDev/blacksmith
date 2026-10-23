import { expect, test, waitForApp } from '../support/console';
import { agentJson, mockTauri } from '../support/tauri';

test.describe('Página del servidor', () => {
    // Fuera de Tauri no hay servidores guardados: un id cualquiera no existe.
    test('ruta dinámica con id codificado: servidor no encontrado y vuelta al panel', async ({ page }) => {
        await page.goto('/servers/deploy%40vps.example.com%3A22');
        await waitForApp(page);
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('Servidor no encontrado');
        await expect(page).toHaveTitle('Servidor no encontrado · Blacksmith');

        await page.getByRole('link', { name: 'Servidores' }).click();
        await expect(page).toHaveURL(/\/$/);
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('Sin servidores todavía');
    });

    test('desde la tarjeta: página del servidor; sin conexión, pide conectar', async ({ page }) => {
        await mockTauri(page, {
            servers_list: [
                { id: 'deploy@vps.example.com:22', kind: 'ssh', host: 'vps.example.com', port: 22, username: 'deploy' },
            ],
        });
        await page.goto('/');
        await waitForApp(page);

        // Clic en la tarjeta (fuera del título): el enlace estirado la cubre entera.
        const card = page.getByRole('article');
        const box = (await card.boundingBox())!;
        await page.mouse.click(box.x + box.width / 2, box.y + box.height - 8);

        await expect(page).toHaveURL(/\/servers\/deploy%40vps\.example\.com%3A22$/);
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('deploy@vps.example.com');
        await expect(page.getByText('Conéctate al servidor para ver y gestionar sus servicios.')).toBeVisible();
        await expect(page.getByRole('main').getByRole('button', { name: 'Conectar' })).toBeVisible();
        await page.screenshot({ path: test.info().outputPath('servidor.png') });

        await page.getByRole('link', { name: 'Servidores' }).click();
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('Servidores');
    });
});

test.describe('Servicios del servidor', () => {
    const LOCAL = { id: 'local:4040', kind: 'local', host: 'localhost', port: 4040, username: '' };
    const SERVICES = [
        {
            name: 'api',
            displayName: 'API de pagos',
            description: 'Cobros.',
            unit: 'blacksmith-api.service',
            createdAt: 0,
        },
    ];
    const NOT_DEPLOYED = {
        name: 'api',
        unit: 'blacksmith-api.service',
        loadState: 'loaded',
        activeState: 'inactive',
        subState: 'dead',
        pid: null,
        metrics: null,
        deployed: false,
        release: null,
        previousRelease: null,
        resources: {
            memoryBytes: null,
            memoryPeakBytes: null,
            memoryLimitBytes: null,
            swapBytes: null,
            cpuPercent: null,
            tasks: null,
            restarts: null,
            uptimeSecs: null,
        },
        host: { cpuCores: 4, memoryBytes: 8589934592, swapBytes: 0 },
    };
    const DEPLOYMENT = {
        name: 'api',
        repository: null,
        workingDirectory: '/srv/blacksmith/api',
        access: 'account',
        setupCommands: ['npm ci'],
        startCommand: 'npm start',
        publishDir: null,
        updatedAt: 1792680000,
    };
    const ENV = {
        name: 'api',
        unit: 'blacksmith-api.service',
        file: '/etc/blacksmith/env/blacksmith-api.env',
        loadedByUnit: true,
        globalLoadedByUnit: true,
        variables: [{ key: 'PORT', value: '8080' }],
    };
    const GLOBAL = { file: '/etc/blacksmith/env/global.env', variables: [{ key: 'PORT', value: '80' }] };

    test('servicio: controles, pestañas de logs, despliegue y variables', async ({ page }) => {
        await mockTauri(page, {
            servers_list: [LOCAL],
            local_agent_status: { name: 'blacksmith-agent', version: '1.0.0', status: 'ready' },
            agent_request: {
                byPath: {
                    'GET /api/v1/services': agentJson(SERVICES),
                    'GET /api/v1/services/api/status': agentJson(NOT_DEPLOYED),
                    'GET /api/v1/services/api/deployment': agentJson(DEPLOYMENT),
                    'GET /api/v1/services/api/env': agentJson(ENV),
                    'GET /api/v1/github': agentJson(null),
                    'GET /api/v1/services/api/site': agentJson(null),
                    'GET /api/v1/services/api/deploys': agentJson([]),
                    'GET /api/v1/env': agentJson(GLOBAL),
                },
            },
            logs_follow: 1,
        });
        await page.goto('/');
        await waitForApp(page);
        await page.getByRole('button', { name: 'Conectar' }).click();
        await expect(page.getByRole('main').getByText('Conectado', { exact: true })).toBeVisible();

        // La barra lateral también enlaza al servidor.
        await expect(page.getByRole('navigation', { name: 'Servidores' }).getByRole('link')).toHaveAccessibleName(
            /localhost:4040/,
        );
        await page.getByRole('main').getByRole('link', { name: 'localhost:4040' }).click();
        await expect(page.getByRole('link', { name: 'API de pagos' })).toBeVisible();
        await page.getByRole('button', { name: 'Registrar servicio' }).click();
        await expect(page.getByRole('dialog', { name: 'Registrar servicio' })).toBeVisible();
        await page.keyboard.press('Escape');

        await page.getByRole('link', { name: 'API de pagos' }).click();
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('API de pagos');
        // Sin desplegar: controles deshabilitados.
        const controls = page.getByRole('region', { name: 'Control del servicio' });
        await expect(controls.getByText('Sin desplegar')).toBeVisible();
        await expect(controls.getByRole('button', { name: 'Iniciar', exact: true })).toBeDisabled();
        await expect(controls.getByRole('button', { name: 'Reiniciar', exact: true })).toBeDisabled();
        // Resumen (pestaña por defecto): sin desplegar, no hay consumo que mostrar.
        const tabs = page.getByRole('navigation', { name: 'Secciones del servicio' });
        await expect(tabs.getByRole('link', { name: 'Resumen' })).toHaveAttribute('aria-current', 'page');
        await expect(page.getByRole('region', { name: 'CPU' })).toBeVisible();
        await expect(page.getByText('El servicio no está en marcha: no consume recursos.')).toBeVisible();
        // Logs.
        await tabs.getByRole('link', { name: 'Logs' }).click();
        await expect(tabs.getByRole('link', { name: 'Logs' })).toHaveAttribute('aria-current', 'page');
        await expect(page.getByText('Aún no hay entradas.')).toBeVisible();

        await tabs.getByRole('link', { name: 'Despliegue' }).click();
        await expect(page).toHaveURL(/\/services\/api\/deployment$/);
        await expect(page.getByText('/srv/blacksmith/api')).toBeVisible();
        await expect(page.getByRole('textbox', { name: 'Comando 1' })).toHaveValue('npm ci');
        await expect(page.getByRole('textbox', { name: /Comando de arranque/ })).toHaveValue('npm start');
        // Sin repositorio no se puede desplegar todavía.
        await expect(page.getByRole('button', { name: 'Conectar con GitHub' })).toBeVisible();
        await expect(
            page.getByText('El servidor no tiene cuenta de GitHub: solo puede desplegar repositorios públicos.'),
        ).toBeVisible();
        await expect(page.getByRole('button', { name: 'Desplegar' })).toBeDisabled();
        await expect(page.getByText('Elige un repositorio y guárdalo.')).toBeVisible();
        await page.screenshot({ path: test.info().outputPath('despliegue.png'), fullPage: true });

        // Web: sin sitio todavía; el formulario propone una aplicación web con HTTPS.
        await tabs.getByRole('link', { name: 'Web' }).click();
        await expect(page).toHaveURL(/\/services\/api\/site$/);
        await expect(page.getByRole('button', { name: 'Tipo de proyecto' })).toHaveText(/Aplicación web/);
        await expect(page.getByRole('switch', { name: /HTTPS/ })).toBeChecked();
        await page.screenshot({ path: test.info().outputPath('web.png'), fullPage: true });

        await tabs.getByRole('link', { name: 'Variables' }).click();
        await expect(page.getByRole('textbox', { name: /^Clave/ })).toHaveValue('PORT');
        await expect(page.getByText('Sobrescrita')).toBeVisible();
        await page.screenshot({ path: test.info().outputPath('variables.png'), fullPage: true });

        await page.getByRole('link', { name: 'Variables globales' }).click();
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('Variables globales del servidor');
        await expect(page.getByRole('textbox', { name: /^Clave/ })).toHaveValue('PORT');
    });
});
