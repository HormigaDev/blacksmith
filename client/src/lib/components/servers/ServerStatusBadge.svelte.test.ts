import { describe, expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import ServerStatusBadge from './ServerStatusBadge.svelte';

describe('ServerStatusBadge', () => {
    test('cada estado con su texto', async () => {
        for (const [status, label] of [
            ['disconnected', 'Desconectado'],
            ['connecting', 'Conectando…'],
            ['connected', 'Conectado'],
            ['agentMissing', 'Agente no configurado'],
            ['error', 'Error'],
        ] as const) {
            const view = await render(ServerStatusBadge, { status });
            await expect.element(page.getByText(label, { exact: true })).toBeVisible();
            await view.unmount();
        }
    });
});
