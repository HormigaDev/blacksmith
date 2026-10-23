import { expect, test, waitForApp } from '../support/console';
import { mockTauri } from '../support/tauri';

test.describe('Página del servidor', () => {
    // Fuera de Tauri no hay servidores guardados: un id cualquiera no existe.
    test('ruta dinámica con id codificado: servidor no encontrado y vuelta al panel', async ({ page }) => {
        await page.goto('/servers/deploy%40vps.example.com%3A22');
        await waitForApp(page);
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('Servidor no encontrado');
        await expect(page).toHaveTitle('Servidor no encontrado · BBEL Blacksmith');

        await page.getByRole('link', { name: 'Servidores' }).click();
        await expect(page).toHaveURL(/\/$/);
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('Sin servidores todavía');
    });

    test('desde la tarjeta: página del servidor con su lista de servicios (vacía)', async ({ page }) => {
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
        await expect(page.getByRole('heading', { level: 2 })).toHaveText('Servicios');
        await expect(page.getByText('Aún no hay servicios')).toBeVisible();
        await expect(page.getByText('Conéctate al servidor para ver y gestionar sus servicios.')).toBeVisible();
        await page.screenshot({ path: test.info().outputPath('servidor.png') });

        await page.getByRole('link', { name: 'Servidores' }).click();
        await expect(page.getByRole('heading', { level: 1 })).toHaveText('Servidores');
    });
});
