import { expect, test, waitForApp } from '../support/console';
import { STORY_COMPONENTS, THEMES } from '../support/stories';
import { applyVisualFonts, prepareVisualPage, uncoveredCharacters } from '../support/visual';

/**
 * Regresión visual: una captura por historia y tema, comparada píxel a píxel (tolerancia cero).
 * El entorno es determinista (e2e/support/visual.ts); tras un cambio visual intencionado,
 * `pnpm test:visual:update` y revisar las capturas en el diff.
 */

test.beforeAll(() => {
    // Windows y macOS rasterizan el texto con otros motores: sus capturas nunca coincidirían con las
    // de referencia. Se falla con una explicación en vez de saltar el test en silencio.
    if (process.platform !== 'linux') {
        throw new Error(
            'La regresión visual solo se ejecuta en Linux (entorno canónico). Usa la CI ' +
                '(.github/workflows/client.yml) o una máquina Linux. Ver client/docs/testing.md.',
        );
    }
});

for (const component of STORY_COMPONENTS) {
    for (const theme of THEMES) {
        test(`${component} (${theme})`, async ({ page }) => {
            await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
            await prepareVisualPage(page);
            await page.goto(`/dev/workbench/${component}`);
            await waitForApp(page);
            // Las historias se cargan de forma diferida: hay que esperar a que aparezcan.
            await expect(page.locator('[data-story]').first()).toBeVisible();
            const slugs = await page
                .locator('[data-story]')
                .evaluateAll((stories) => stories.map((story) => story.id).filter(Boolean));
            expect(slugs.length, 'el componente debe tener historias').toBeGreaterThan(0);

            // Cada historia se captura AISLADA (`?story=`): así su captura no depende de las que tenga
            // encima (una historia nueva desplazaría a las demás a posiciones fraccionarias y el
            // recorte cambiaría 1 px sin que nada visual haya cambiado).
            for (const slug of slugs) {
                await page.goto(`/dev/workbench/${component}?story=${slug}`);
                await waitForApp(page);
                await applyVisualFonts(page);
                const story = page.locator('[data-story]');
                await expect(story).toHaveCount(1);
                expect(await uncoveredCharacters(page, '[data-story]'), 'caracteres sin fuente versionada').toEqual([]);
                await expect(story).toHaveScreenshot(`${component}--${slug}-${theme}.png`);
            }
        });
    }
}
