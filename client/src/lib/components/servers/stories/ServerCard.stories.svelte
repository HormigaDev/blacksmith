<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'ServerCard',
        group: 'Servidores',
        description: 'Un servidor configurado y el estado de su conexión.',
    };
</script>

<script lang="ts">
    import type { ServerEntry } from '$lib/servers/servers.svelte';
    import { Story } from '$lib/workbench';

    import ServerCard from '../ServerCard.svelte';

    const profile = {
        id: 'deploy@vps.example.com:22',
        kind: 'ssh' as const,
        host: 'vps.example.com',
        port: 22,
        username: 'deploy',
    };
    const local = { id: 'local:4040', kind: 'local' as const, host: 'localhost', port: 4040, username: '' };
    const agent = { name: 'blacksmith-agent', version: '1.0.0', status: 'ready' as const };
    const states: { name: string; entry: ServerEntry }[] = [
        { name: 'Desconectado', entry: { profile, status: 'disconnected' } },
        { name: 'Conectando', entry: { profile, status: 'connecting' } },
        {
            name: 'Conectado',
            entry: {
                profile,
                status: 'connected',
                tunnel: { targetId: profile.id, localPort: 51234, remotePort: 4040, agent },
            },
        },
        { name: 'Agente no configurado', entry: { profile, status: 'agentMissing' } },
        {
            name: 'Error',
            entry: {
                profile: { ...profile, port: 2222, id: 'deploy@vps.example.com:2222' },
                status: 'error',
                error: 'No se pudo contactar con el servidor. Revisa el host, el puerto y tu conexión.',
                errorCode: 'HOST_UNREACHABLE',
            },
        },
        {
            name: 'Llave de servidor cambiada',
            entry: {
                profile,
                status: 'error',
                error: 'La llave del servidor cambió desde la última conexión. Podría tratarse de un ataque: no se ha conectado.',
                errorCode: 'HOST_KEY_CHANGED',
            },
        },
        {
            name: 'Local conectado',
            entry: {
                profile: local,
                status: 'connected',
                tunnel: { targetId: local.id, localPort: 4040, remotePort: 4040, agent },
            },
        },
        { name: 'Local sin agente', entry: { profile: local, status: 'agentMissing' } },
    ];
    const noop = () => {};
</script>

{#each states as { name, entry } (name)}
    <Story {name} layout="stack">
        <div class="frame">
            <ServerCard {entry} onConnect={noop} onDisconnect={noop} onRemove={noop} onForgetHostKey={noop} />
        </div>
    </Story>
{/each}

<Story name="Tarjeta de cuadrícula" layout="grid">
    {#each states.slice(0, 3) as { name, entry } (name)}
        <div class="tile">
            <ServerCard
                {entry}
                layout="tile"
                onConnect={noop}
                onDisconnect={noop}
                onRemove={noop}
                onForgetHostKey={noop}
            />
        </div>
    {/each}
</Story>

<style>
    .tile {
        width: var(--layout-card-min-width);
    }

    .frame {
        width: min(100%, var(--layout-content-max-width));
    }
</style>
