<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'FilePicker',
        group: 'Formularios',
        description:
            'Devuelve rutas absolutas. Con `pnpm workbench` (Tauri) usa el diálogo nativo y el arrastrar y soltar real.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import FilePicker from '../FilePicker.svelte';
</script>

<Story name="Archivo" layout="grid">
    <FilePicker label="Llave SSH" accept={['pem', 'key', 'pub']} hint="Solo .pem, .key o .pub" />
    <FilePicker label="Varios archivos" multiple />
</Story>

<Story name="Carpeta" layout="grid">
    <FilePicker label="Directorio del proyecto" directory />
</Story>

<Story name="Con valor" layout="grid">
    <FilePicker label="Llaves" multiple value={['/home/ops/.ssh/deploy.pem', '/home/ops/.ssh/backup.pem']} />
</Story>

<Story name="Estados" layout="grid">
    <FilePicker label="Con error" error="La llave no tiene permisos 600" />
    <FilePicker label="Deshabilitado" disabled />
</Story>
