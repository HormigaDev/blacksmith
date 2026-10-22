<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'AddServerForm',
        group: 'Servidores',
        description:
            'Alta de un servidor. Con `pnpm workbench` (Tauri) el envío llega a Rust; en el navegador, solo la interfaz.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import AddServerForm from '../AddServerForm.svelte';
</script>

<Story name="Vacío" layout="stack">
    <div class="frame"><AddServerForm /></div>
</Story>

<Story name="Con destino" layout="stack">
    <div class="frame">
        <AddServerForm initial={{ host: 'vps.example.com', port: 2222, username: 'deploy' }} />
    </div>
</Story>

<Story name="Este equipo" layout="stack" description="Con localhost no hay túnel ni llave: solo el puerto del agente.">
    <div class="frame">
        <AddServerForm initial={{ host: 'localhost' }} />
    </div>
</Story>

<style>
    .frame {
        width: min(100%, var(--dialog-width-lg));
    }
</style>
