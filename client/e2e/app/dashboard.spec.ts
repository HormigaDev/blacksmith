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

    // Ventana estrecha o móvil (styles.md, «Tamaños de ventana»): la barra lateral es un panel.
    test('compacto: la navegación se abre con el menú y se cierra con Escape', async ({ page }) => {
        await page.setViewportSize({ width: 400, height: 700 });
        await page.goto('/');
        await waitForApp(page);

        const home = page.getByRole('link', { name: 'Servidores' });
        const menu = page.getByRole('button', { name: 'Mostrar navegación' });
        await expect(home).toBeHidden();
        await menu.click();
        await expect(home).toBeVisible();
        await expect(home).toBeFocused();
        await expect(page.getByRole('button', { name: 'Ocultar navegación' })).toHaveAttribute('aria-expanded', 'true');

        await page.keyboard.press('Escape');
        await expect(home).toBeHidden();
        await expect(menu).toBeFocused();
        expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(400);
    });

    test('ancho normal: sin botón de menú y con la barra lateral fija', async ({ page }) => {
        await page.goto('/');
        await waitForApp(page);
        await expect(page.getByRole('button', { name: 'Mostrar navegación' })).toBeHidden();
        await expect(page.getByRole('link', { name: 'Servidores' })).toBeVisible();
    });
});
