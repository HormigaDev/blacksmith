<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'Panel',
        group: 'Estado',
        description: 'Sección de una página con título y acciones opcionales.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import Button from '../Button.svelte';
    import Panel from '../Panel.svelte';
</script>

<Story name="Con título y acciones" layout="stack">
    <div class="frame">
        <Panel title="Despliegues">
            {#snippet actions()}<Button size="sm">Acción</Button>{/snippet}
            <p class="text">Contenido de la sección.</p>
        </Panel>
    </div>
</Story>

<Story name="Sin título" layout="stack">
    <div class="frame">
        <Panel><p class="text">Contenido de la sección.</p></Panel>
    </div>
</Story>

<style>
    .frame {
        width: min(100%, var(--dialog-width-lg));
    }

    .text {
        font-size: var(--font-size-sm);
    }
</style>
