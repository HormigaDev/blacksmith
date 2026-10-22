<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'Tooltip',
        group: 'Superposiciones',
        description: 'Al pasar el ratón o enfocar.',
    };
</script>

<script lang="ts">
    import InfoIcon from '@lucide/svelte/icons/info';

    import { Story } from '$lib/workbench';

    import Button from '../Button.svelte';
    import Tooltip from '../Tooltip.svelte';
</script>

<Story name="Posiciones" layout="centered">
    {#each ['top', 'right', 'bottom', 'left'] as const as side (side)}
        <Tooltip text="Reinicia el servicio sin cortar conexiones" {side}>
            {#snippet children(props)}
                <Button {...props}>{side}</Button>
            {/snippet}
        </Tooltip>
    {/each}
</Story>

<Story name="En un botón de ícono" layout="centered">
    <Tooltip text="Más información">
        {#snippet children(props)}
            <Button {...props} iconOnly variant="ghost" aria-label="Más información"><InfoIcon /></Button>
        {/snippet}
    </Tooltip>
</Story>
