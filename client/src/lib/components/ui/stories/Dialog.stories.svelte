<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'Dialog',
        group: 'Superposiciones',
        description: 'Modal con foco atrapado.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import Button from '../Button.svelte';
    import ConfirmDialog from '../ConfirmDialog.svelte';
    import Dialog from '../Dialog.svelte';
    import Input from '../Input.svelte';

    const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

    let form = $state(false);
    let large = $state(false);
    let confirm = $state(false);
    let danger = $state(false);
</script>

<Story name="Formulario">
    <Button onclick={() => (form = true)}>Nuevo servidor</Button>
    <Dialog bind:open={form} title="Nuevo servidor" description="Se conectará por túnel SSH.">
        <div class="stack">
            <Input label="Host" placeholder="vps.example.com" />
            <Input label="Usuario" placeholder="deploy" />
        </div>
        {#snippet footer()}
            <Button onclick={() => (form = false)}>Cancelar</Button>
            <Button variant="primary" onclick={() => (form = false)}>Guardar</Button>
        {/snippet}
    </Dialog>
</Story>

<Story name="Tamaño grande">
    <Button onclick={() => (large = true)}>Ver detalles</Button>
    <Dialog bind:open={large} size="lg" title="api.service">
        <p>Contenido extenso: variables, historial de despliegues, etc.</p>
    </Dialog>
</Story>

<Story name="Confirmación" description="ConfirmDialog: obligatorio antes de una acción destructiva.">
    <Button onclick={() => (confirm = true)}>Desplegar</Button>
    <Button variant="danger" onclick={() => (danger = true)}>Detener servicio</Button>
    <ConfirmDialog
        bind:open={confirm}
        title="¿Desplegar main en producción?"
        description="Se reiniciará api.service al terminar."
        confirmLabel="Desplegar"
        onConfirm={() => sleep(1000)}
    />
    <ConfirmDialog
        bind:open={danger}
        variant="danger"
        title="¿Detener api.service?"
        description="El servicio dejará de atender peticiones hasta que vuelvas a iniciarlo."
        confirmLabel="Detener"
        onConfirm={() => sleep(1000)}
    />
</Story>

<style>
    .stack {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
    }
</style>
