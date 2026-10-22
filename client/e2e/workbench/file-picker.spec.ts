import { expect, test, waitForApp } from '../support/console';
import { emitTauriEvent, mockTauri } from '../support/tauri';

// Tauri intercepta el drop de HTML5 y entrega rutas + posición por eventos nativos. Se emulan aquí.
test.describe('FilePicker con arrastrar y soltar nativo', () => {
    test.beforeEach(async ({ page }) => {
        await mockTauri(page);
        await page.goto('/dev/workbench/file-picker?story=archivo');
        await waitForApp(page);
    });

    async function centerOf(page: import('@playwright/test').Page, label: string) {
        const box = await page.getByRole('group', { name: label }).boundingBox();
        if (!box) throw new Error(`sin zona para ${label}`);
        return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
    }

    test('resalta la zona al pasar por encima y añade los archivos al soltar', async ({ page }) => {
        const zone = page.getByRole('group', { name: 'Llave SSH' });
        const position = await centerOf(page, 'Llave SSH');

        await emitTauriEvent(page, 'tauri://drag-enter', { paths: ['/home/ops/.ssh/deploy.pem'], position });
        await expect(zone).toContainText('Suelta para añadir');

        await emitTauriEvent(page, 'tauri://drag-drop', { paths: ['/home/ops/.ssh/deploy.pem'], position });
        await expect(page.getByText('deploy.pem')).toBeVisible();
        await expect(zone).not.toContainText('Suelta para añadir');
    });

    test('ignora lo que se suelta fuera de la zona', async ({ page }) => {
        await emitTauriEvent(page, 'tauri://drag-drop', { paths: ['/tmp/x.pem'], position: { x: 1, y: 1 } });
        await expect(page.getByText('x.pem')).toHaveCount(0);
    });

    test('rechaza formatos no admitidos', async ({ page }) => {
        const position = await centerOf(page, 'Llave SSH');
        await emitTauriEvent(page, 'tauri://drag-drop', { paths: ['/tmp/notas.txt'], position });
        await expect(page.getByRole('alert')).toHaveText('1 archivo no tiene un formato admitido');
    });
});
