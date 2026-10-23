<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'Meter',
        group: 'Datos',
        description: 'Proporción frente a un límite: normal, alto (≥ 75 %) y crítico (≥ 90 %).',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import Meter from '../Meter.svelte';
</script>

<Story name="Niveles" layout="stack">
    <div class="frame">
        <Meter label="Normal" value={35} max={100} />
        <Meter label="Alto" value={80} max={100} />
        <Meter label="Crítico" value={96} max={100} />
    </div>
</Story>

<style>
    .frame {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
        width: var(--layout-card-min-width);
    }
</style>
