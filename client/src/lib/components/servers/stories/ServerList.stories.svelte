<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'ServerList',
        group: 'Servidores',
        description: 'Servidores en lista o cuadrícula, reordenables con el asa o el teclado.',
    };
</script>

<script lang="ts">
    import type { ServerEntry } from '$lib/servers/servers.svelte';
    import { Story } from '$lib/workbench';

    import ServerList from '../ServerList.svelte';

    const ssh = (host: string, status: ServerEntry['status']): ServerEntry => ({
        profile: { id: `deploy@${host}:22`, kind: 'ssh', host, port: 22, username: 'deploy' },
        status,
    });
    const initial: ServerEntry[] = [
        ssh('produccion.example.com', 'connected'),
        ssh('staging.example.com', 'disconnected'),
        {
            profile: { id: 'local:4040', kind: 'local', host: 'localhost', port: 4040, username: '' },
            status: 'agentMissing',
        },
    ];
    initial[0] = {
        ...initial[0]!,
        tunnel: {
            targetId: initial[0]!.profile.id,
            localPort: 51234,
            remotePort: 4040,
            agent: { name: 'blacksmith-agent', version: '1.0.0', status: 'ready' },
        },
    };

    let list = $state([...initial]);
    let grid = $state([...initial]);

    const reorder = (entries: ServerEntry[], from: number, to: number): ServerEntry[] => {
        const next = [...entries];
        const [moved] = next.splice(from, 1);
        if (moved) next.splice(to, 0, moved);
        return next;
    };
    const noop = () => {};
</script>

<Story name="Lista" layout="stack">
    <div class="frame">
        <ServerList
            entries={list}
            view="list"
            onMove={(from, to) => (list = reorder(list, from, to))}
            onCommit={noop}
            onConnect={noop}
            onDisconnect={noop}
            onRemove={noop}
            onForgetHostKey={noop}
        />
    </div>
</Story>

<Story name="Cuadrícula" layout="stack">
    <div class="frame">
        <ServerList
            entries={grid}
            view="grid"
            onMove={(from, to) => (grid = reorder(grid, from, to))}
            onCommit={noop}
            onConnect={noop}
            onDisconnect={noop}
            onRemove={noop}
            onForgetHostKey={noop}
        />
    </div>
</Story>

<style>
    .frame {
        width: min(100%, var(--layout-content-max-width));
    }
</style>
