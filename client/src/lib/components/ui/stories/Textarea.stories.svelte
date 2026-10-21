<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'Textarea',
        group: 'Formularios',
        description: 'Crece con el contenido hasta maxRows; con maxlength muestra un contador.',
    };
</script>

<script lang="ts">
    import SparklesIcon from '@lucide/svelte/icons/sparkles';

    import { Story } from '$lib/workbench';

    import Textarea from '../Textarea.svelte';

    let notes = $state('Despliegue de la rama main.\nIncluye migraciones.');
</script>

<Story name="Básico" layout="grid">
    <Textarea label="Notas" placeholder="Escribe algo…" />
    <Textarea label="Con contenido" bind:value={notes} hint="Crece al escribir" />
</Story>

<Story name="Contador e ícono" layout="grid">
    <Textarea label="Mensaje del commit" maxlength={72} rows={2} />
    <Textarea label="Con ícono" placeholder="Describe el cambio…">
        {#snippet trailing()}<SparklesIcon />{/snippet}
    </Textarea>
</Story>

<Story name="Monoespaciado" layout="grid" description="Para llaves, configuración o logs.">
    <Textarea
        label="Llave privada"
        monospace
        rows={4}
        value={'-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\n-----END OPENSSH PRIVATE KEY-----'}
    />
</Story>

<Story name="Estados" layout="grid">
    <Textarea label="Con error" value="x" error="Describe el motivo con más detalle" />
    <Textarea label="Deshabilitado" value="No editable" disabled />
    <Textarea label="Máximo 4 filas" maxRows={4} value={'línea\n'.repeat(8)} />
</Story>
