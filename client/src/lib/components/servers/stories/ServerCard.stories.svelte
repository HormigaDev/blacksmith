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

    const profile = { id: 'deploy@vps.example.com:22', host: 'vps.example.com', port: 22, username: 'deploy' };
    const states: { name: string; entry: ServerEntry }[] = [
        { name: 'Desconectado', entry: { profile, status: 'disconnected' } },
        { name: 'Conectando', entry: { profile, status: 'connecting' } },
        {
            name: 'Conectado',
            entry: {
                profile,
                status: 'connected',
                tunnel: { targetId: profile.id, localPort: 51234, remotePort: 4040 },
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

<style>
    .frame {
        width: min(100%, var(--layout-content-max-width));
    }
</style>
