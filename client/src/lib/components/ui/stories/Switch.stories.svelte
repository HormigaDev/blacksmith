<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'Switch',
        group: 'Selección',
        description: 'Ajustes que se aplican al momento.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import Switch from '../Switch.svelte';
</script>

<Story name="Estados" layout="stack">
    <Switch label="Apagado" />
    <Switch label="Encendido" checked />
    <Switch label="Deshabilitado" disabled />
    <Switch label="Deshabilitado y encendido" disabled checked />
</Story>

<Story name="Con descripción" layout="stack">
    <Switch label="Despliegue automático" description="Desplegar al hacer push a la rama configurada." checked />
</Story>
