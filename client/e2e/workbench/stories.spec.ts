import { expect, test, waitForApp } from '../support/console';
import { STORY_COMPONENTS } from '../support/stories';

test('el menú lista exactamente los componentes con historias', async ({ page }) => {
    await page.goto('/dev/workbench');
    await waitForApp(page);
    // El menú se rellena al cargar los metadatos de las historias (asíncrono).
    await expect(page.locator('[data-component]')).toHaveCount(STORY_COMPONENTS.length);
    const listed = await page
        .locator('[data-component]')
        .evaluateAll((links) => links.map((link) => link.getAttribute('data-component')));
    expect(listed.sort()).toEqual([...STORY_COMPONENTS]);
});

for (const component of STORY_COMPONENTS) {
    test(`${component}: todas las historias se renderizan sin errores`, async ({ page }) => {
        await page.goto(`/dev/workbench/${component}`);
        await waitForApp(page);
        const stories = page.locator('[data-story]');
        await expect(stories.first()).toBeVisible();
        for (const story of await stories.all()) {
            // Por elementos y no por texto: hay historias que solo contienen íconos.
            await expect(story.locator('[data-story-canvas] > *')).not.toHaveCount(0);
        }
    });
}

test('?story= aísla una sola variante', async ({ page }) => {
    await page.goto('/dev/workbench/input?story=numero');
    await waitForApp(page);
    await expect(page.locator('[data-story]')).toHaveCount(1);
    await expect(page.locator('[data-story]')).toHaveAttribute('data-story', 'input--numero');
});
