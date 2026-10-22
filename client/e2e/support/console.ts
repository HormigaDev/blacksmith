import { expect, type Page, test as base } from '@playwright/test';

/**
 * `test` con vigilancia de la consola: cualquier error de JavaScript o `console.error` durante el
 * test lo hace fallar. Un error en consola es un fallo, aunque la pantalla "se vea bien".
 */
export const test = base.extend<{ consoleErrors: string[] }>({
    consoleErrors: [
        async ({ page }, use) => {
            const errors: string[] = [];
            page.on('pageerror', (error) => errors.push(`pageerror: ${error.message}`));
            page.on('console', (message) => {
                if (message.type() === 'error') errors.push(`console.error: ${message.text()}`);
            });
            await use(errors);
            expect(errors, 'errores en la consola del navegador').toEqual([]);
        },
        { auto: true },
    ],
});

export { expect };

/** Espera a que SvelteKit haya hidratado la app (la SPA no tiene HTML útil antes). */
export async function waitForApp(page: Page): Promise<void> {
    await page.waitForFunction(() => document.querySelector('body > div')?.childElementCount);
}
