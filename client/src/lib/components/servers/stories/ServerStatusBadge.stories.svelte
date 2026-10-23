<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'ServerStatusBadge',
        group: 'Servidores',
        description: 'Estado de la conexión con un servidor.',
    };
</script>

<script lang="ts">
    import type { ServerEntry } from '$lib/servers/servers.svelte';
    import { Story } from '$lib/workbench';

    import ServerStatusBadge from '../ServerStatusBadge.svelte';

    const statuses: ServerEntry['status'][] = ['disconnected', 'connecting', 'connected', 'agentMissing', 'error'];
</script>

<Story name="Estados">
    {#each statuses as status (status)}
        <ServerStatusBadge {status} />
    {/each}
</Story>
