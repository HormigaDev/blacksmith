import { expect, test, waitForApp } from '../support/console';

test.describe('Ventana principal', () => {
    test('carga en español con el título y el idioma del documento', async ({ page }) => {
        await page.goto('/');
        await waitForApp(page);
        await expect(page).toHaveTitle('Panel · BBEL Blacksmith');
        await expect(page.locator('html')).toHaveAttribute('lang', 'es');
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('Sin servidores todavía');
    });

    test('sigue el tema del sistema desde el primer pintado', async ({ page }) => {
        await page.emulateMedia({ colorScheme: 'dark' });
        await page.goto('/');
        // El script de app.html lo fija antes de hidratar: sin destello del tema claro.
        await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
        await waitForApp(page);
        await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
    });

    test('el tema elegido se aplica al momento y se recuerda', async ({ page }) => {
        await page.emulateMedia({ colorScheme: 'light' });
        await page.goto('/');
        await waitForApp(page);

        await page.getByRole('button', { name: 'Tema' }).click();
        await page.getByRole('option', { name: 'Oscuro' }).click();
        await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
        const background = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);

        await page.reload();
        await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
        await expect.poll(() => page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe(background);
    });

    test('el selector de idioma solo ofrece idiomas disponibles', async ({ page }) => {
        await page.goto('/');
        await waitForApp(page);
        await page.getByRole('button', { name: 'Idioma' }).click();
        await expect(page.getByRole('option')).toHaveText(['Sistema', 'Español']);
    });

    test('las herramientas de desarrollo no existen en producción', async ({ page }) => {
        await page.goto('/dev/workbench');
        await waitForApp(page);
        await expect(page.getByRole('heading', { name: '404' })).toBeVisible();
    });
});
