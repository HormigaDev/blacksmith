import { expect, test, waitForApp } from '../support/console';
import { mockTauri, tauriCalls } from '../support/tauri';

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
});
