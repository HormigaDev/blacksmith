<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'GitHubMenu',
        group: 'GitHub',
        description: 'Botón de GitHub de la cabecera (abre el panel de conexión).',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import GitHubMenu from '../GitHubMenu.svelte';
</script>

<Story name="Sin conectar">
    <GitHubMenu preview />
</Story>
