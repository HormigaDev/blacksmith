<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'ToggleButton',
        group: 'Acciones',
        description: 'Botón con estado activado/desactivado.',
    };
</script>

<script lang="ts">
    import PinIcon from '@lucide/svelte/icons/pin';
    import RadioIcon from '@lucide/svelte/icons/radio';
    import WrapTextIcon from '@lucide/svelte/icons/wrap-text';

    import { Story } from '$lib/workbench';

    import ToggleButton from '../ToggleButton.svelte';
</script>

<Story name="Estados">
    <ToggleButton pressed><RadioIcon /> Seguir logs</ToggleButton>
    <ToggleButton><WrapTextIcon /> Ajustar líneas</ToggleButton>
    <ToggleButton disabled><PinIcon /> Fijar</ToggleButton>
</Story>

<Story name="Solo ícono">
    <ToggleButton aria-label="Seguir logs" pressed><RadioIcon /></ToggleButton>
    <ToggleButton aria-label="Ajustar líneas" size="sm"><WrapTextIcon /></ToggleButton>
</Story>
