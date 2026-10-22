import AxeBuilder from '@axe-core/playwright';

import { expect, test, waitForApp } from '../support/console';
import { STORY_COMPONENTS, THEMES } from '../support/stories';

/**
 * Accesibilidad de cada componente, en cada tema: WCAG 2.1 A y AA, incluido el contraste de color.
 * Solo se analizan las historias ([data-story]), no la interfaz del propio Workbench.
 */
for (const component of STORY_COMPONENTS) {
    for (const theme of THEMES) {
        test(`${component} (${theme}): sin violaciones de accesibilidad`, async ({ page }) => {
            await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
            await page.goto(`/dev/workbench/${component}`);
            await waitForApp(page);
            await expect(page.locator('[data-story]').first()).toBeVisible();

            const results = await new AxeBuilder({ page })
                .include('[data-story-canvas]')
                .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
                .analyze();

            const violations = results.violations.map(({ id, impact, help, nodes }) => ({
                id,
                impact,
                help,
                targets: nodes.map((node) => node.target.join(' ')),
            }));
            expect(violations).toEqual([]);
        });
    }
}
