import { describe, expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import { github } from '$lib/github';

import { mockTauri } from '../../../tests/tauri';
import GitHubMenu from './GitHubMenu.svelte';

describe('GitHubMenu', () => {
    test('fuera de Tauri no se muestra', async () => {
        await render(GitHubMenu);
        await expect.element(page.getByRole('button', { name: /GitHub/ })).not.toBeInTheDocument();
    });

    test('muestra la cuenta conectada y abre el panel', async () => {
        const { calls } = mockTauri({
            github_status: () => ({
                settings: { clientId: 'Iv23liAbCdEf123456', appSlug: 'mi-blacksmith' },
                user: { login: 'octocat', name: null, avatarUrl: null },
            }),
        });
        await github.load();
        await render(GitHubMenu);
        await page.getByRole('button', { name: 'GitHub: octocat' }).click();
        await expect.element(page.getByRole('dialog', { name: 'GitHub' })).toBeVisible();
        await expect.element(page.getByText('octocat', { exact: true })).toBeVisible();
        expect(calls.some((call) => call.cmd === 'github_status')).toBe(true);
    });
});
