<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = { title: 'Checkbox', group: 'Selección' };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import Checkbox from '../Checkbox.svelte';

    let checked = $state(true);
</script>

<Story name="Estados" layout="stack">
    <Checkbox label="Sin marcar" />
    <Checkbox label="Marcado" bind:checked />
    <Checkbox label="Mixto" indeterminate />
    <Checkbox label="Deshabilitado" disabled />
    <Checkbox label="Deshabilitado y marcado" disabled checked />
</Story>

<Story name="Con descripción" layout="stack">
    <Checkbox label="Reiniciar tras desplegar" description="El servicio estará caído unos segundos." checked />
</Story>
