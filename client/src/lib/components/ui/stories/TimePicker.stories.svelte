<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'TimePicker',
        group: 'Formularios',
        description: 'Campo segmentado: dígitos y flechas. Valor ISO HH:mm o HH:mm:ss.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import TimePicker from '../TimePicker.svelte';

    let time = $state<string | undefined>('09:30');
</script>

<Story name="Básico" layout="grid">
    <TimePicker label="Hora" bind:value={time} hint={`value = ${time ?? '—'}`} />
    <TimePicker label="Vacío" />
</Story>

<Story name="Formatos" layout="grid">
    <TimePicker label="24 horas" hourCycle={24} value="18:45" />
    <TimePicker label="12 horas" hourCycle={12} value="18:45" />
    <TimePicker label="Con segundos" granularity="second" value="03:15:42" />
</Story>

<Story name="Estados" layout="grid">
    <TimePicker label="Con error" error="Fuera de la ventana de mantenimiento" value="13:00" />
    <TimePicker label="Deshabilitado" value="02:00" disabled />
</Story>
