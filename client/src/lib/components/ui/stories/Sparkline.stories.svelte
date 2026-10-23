<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'Sparkline',
        group: 'Datos',
        description: 'Tendencia de una serie con lectura al pasar el puntero o con el teclado.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import Sparkline from '../Sparkline.svelte';

    const start = 1_800_000_000_000;
    const values = [12, 14, 13, 18, 25, 31, 28, 22, 19, 21, 35, 42, 38, 30, 27, 24, 22, 20, 23, 26];
    const times = values.map((_, index) => start + index * 3000);
    const withGap = values.map((value, index) => (index > 8 && index < 12 ? null : value));
    const format = (value: number) => `${value} %`;
</script>

<Story name="Serie" layout="stack">
    <div class="frame"><Sparkline label="CPU" {values} {times} {format} /></div>
</Story>

<Story name="Con un hueco (parado)" layout="stack">
    <div class="frame"><Sparkline label="CPU" values={withGap} {times} {format} /></div>
</Story>

<Story name="Sin datos" layout="stack">
    <div class="frame"><Sparkline label="CPU" values={[]} {format} /></div>
</Story>

<style>
    .frame {
        width: var(--layout-card-min-width);
    }
</style>
