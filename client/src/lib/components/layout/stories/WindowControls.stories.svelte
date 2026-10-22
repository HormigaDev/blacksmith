<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'WindowControls',
        group: 'Layout',
        description: 'Minimizar, maximizar y cerrar en la ventana sin marco nativo.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import WindowControls from '../WindowControls.svelte';
</script>

<Story name="Barra de título">
    <div class="bar">
        <WindowControls preview />
    </div>
</Story>

<style>
    .bar {
        display: flex;
        justify-content: flex-end;
        width: 100%;
        height: var(--layout-header-height);
        background-color: var(--color-surface);
        border: var(--border-width) solid var(--color-border);
    }
</style>
