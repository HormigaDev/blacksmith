<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'DatePicker',
        group: 'Formularios',
        description: 'Campo segmentado + calendario. Valor ISO AAAA-MM-DD; formato según el idioma.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import DatePicker from '../DatePicker.svelte';

    let date = $state<string | undefined>('2026-10-21');
</script>

<Story name="Básico" layout="grid">
    <DatePicker label="Fecha de despliegue" hint="Escribe o usa el calendario" />
    <DatePicker label="Con valor" bind:value={date} hint={`value = ${date ?? '—'}`} />
</Story>

<Story name="Límites" layout="grid">
    <DatePicker label="Solo octubre de 2026" min="2026-10-01" max="2026-10-31" />
</Story>

<Story name="Estados" layout="grid">
    <DatePicker label="Obligatorio" required />
    <DatePicker label="Con error" error="La fecha ya pasó" value="2026-01-01" />
    <DatePicker label="Deshabilitado" value="2026-10-21" disabled />
</Story>
