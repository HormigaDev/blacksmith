import { describe, expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { ServerEntry } from '$lib/servers/servers.svelte';

import ServerNav from './ServerNav.svelte';

const vps = {
    id: 'deploy@vps.example.com:22',
    kind: 'ssh' as const,
    host: 'vps.example.com',
    port: 22,
    username: 'deploy',
};
const local = { id: 'local:4040', kind: 'local' as const, host: 'localhost', port: 4040, username: '' };
const entries: ServerEntry[] = [
    { profile: vps, status: 'disconnected' },
    { profile: local, status: 'connecting' },
];

describe('ServerNav', () => {
    test('enlaza cada servidor con su estado y marca el abierto', async () => {
        await render(ServerNav, { entries, currentPath: `/servers/${encodeURIComponent(local.id)}/services/api` });
        const links = page.getByRole('navigation').getByRole('link');
        await expect.element(links.nth(0)).toHaveAccessibleName(/deploy@vps\.example\.com/);
        await expect.element(links.nth(0)).toHaveAccessibleName(/Desconectado/);
        await expect.element(links.nth(0)).not.toHaveAttribute('aria-current');
        await expect.element(links.nth(1)).toHaveAttribute('aria-current', 'page');
    });
});
