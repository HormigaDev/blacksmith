<!--
    Confirmación de una acción. Obligatoria antes de cualquier acción destructiva (parar un
    servicio, borrar variables, desplegar). No se cierra al pulsar fuera: hay que decidir.
    Si `onConfirm` lanza un error, el diálogo sigue abierto y el llamador decide cómo avisar.
-->
<script lang="ts">
    import { AlertDialog } from 'bits-ui';
    import type { Snippet } from 'svelte';

    import { t } from '$lib/i18n';

    import Button from './Button.svelte';

    interface Props {
        open?: boolean;
        title: string;
        description: string;
        confirmLabel?: string;
        cancelLabel?: string;
        /** `danger` para acciones destructivas o irreversibles. */
        variant?: 'primary' | 'danger';
        onConfirm: () => void | Promise<void>;
        /** Contenido adicional (p. ej. un resumen de lo que se va a hacer). */
        children?: Snippet;
    }

    let {
        open = $bindable(false),
        title,
        description,
        confirmLabel,
        cancelLabel,
        variant = 'primary',
        onConfirm,
        children,
    }: Props = $props();

    let busy = $state(false);

    async function confirm(): Promise<void> {
        busy = true;
        try {
            await onConfirm();
            open = false;
        } finally {
            busy = false;
        }
    }
</script>

<!-- Mientras se ejecuta la acción no se puede cerrar (ni con Escape). -->
<AlertDialog.Root
    bind:open={
        () => open,
        (next) => {
            if (!busy) open = next;
        }
    }
>
    <AlertDialog.Portal>
        <AlertDialog.Overlay class="bs-dialog-overlay" />
        <AlertDialog.Content class="bs-dialog bs-dialog-sm">
            <header>
                <AlertDialog.Title class="bs-dialog-title">{title}</AlertDialog.Title>
            </header>
            <AlertDialog.Description class="bs-dialog-description">{description}</AlertDialog.Description>

            {#if children}
                <div class="bs-dialog-body">{@render children()}</div>
            {/if}

            <footer class="bs-dialog-footer">
                <AlertDialog.Cancel disabled={busy}>
                    {#snippet child({ props })}
                        <Button {...props}>{cancelLabel ?? t('common.cancel')}</Button>
                    {/snippet}
                </AlertDialog.Cancel>
                <Button variant={variant === 'danger' ? 'danger' : 'primary'} loading={busy} onclick={confirm}>
                    {confirmLabel ?? t('common.confirm')}
                </Button>
            </footer>
        </AlertDialog.Content>
    </AlertDialog.Portal>
</AlertDialog.Root>
