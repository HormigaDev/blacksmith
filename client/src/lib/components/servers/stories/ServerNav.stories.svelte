<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'ServerNav',
        group: 'Servidores',
        description: 'Servidores en la barra lateral, con su estado.',
    };
</script>

<script lang="ts">
    import type { ServerEntry } from '$lib/servers/servers.svelte';
    import { Story } from '$lib/workbench';

    import ServerNav from '../ServerNav.svelte';

    const profile = (host: string) => ({
        id: `deploy@${host}:22`,
        kind: 'ssh' as const,
        host,
        port: 22,
        username: 'deploy',
    });
    const entries: ServerEntry[] = [
        { profile: profile('vps.example.com'), status: 'disconnected' },
        { profile: profile('api.example.com'), status: 'connecting' },
        { profile: profile('db.example.com'), status: 'agentMissing' },
        { profile: profile('old.example.com'), status: 'error', error: 'Conexión rechazada.' },
    ];
</script>

<Story name="Varios estados" layout="stack">
    <div class="frame"><ServerNav {entries} currentPath="/servers/deploy%40api.example.com%3A22" /></div>
</Story>

<Story name="Vacía" layout="stack">
    <div class="frame"><ServerNav entries={[]} currentPath="/" /></div>
</Story>

<style>
    .frame {
        width: var(--layout-sidebar-width);
        padding: var(--space-3);
        background-color: var(--color-bg-subtle);
    }
</style>
