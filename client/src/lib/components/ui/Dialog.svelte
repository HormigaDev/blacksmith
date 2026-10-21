<!-- Ventana modal: foco atrapado, Escape para cerrar y fondo inerte. -->
<script lang="ts">
    import XIcon from '@lucide/svelte/icons/x';
    import { Dialog } from 'bits-ui';
    import type { Snippet } from 'svelte';

    import { t } from '$lib/i18n';

    interface Props {
        open?: boolean;
        title: string;
        description?: string;
        size?: 'sm' | 'md' | 'lg';
        /** Acciones al pie (botones). */
        footer?: Snippet;
        children?: Snippet;
        onOpenChange?: (open: boolean) => void;
    }

    let { open = $bindable(false), title, description, size = 'md', footer, children, onOpenChange }: Props = $props();
</script>

<Dialog.Root bind:open {onOpenChange}>
    <Dialog.Portal>
        <Dialog.Overlay class="bs-dialog-overlay" />
        <Dialog.Content class="bs-dialog {size === 'md' ? '' : `bs-dialog-${size}`}">
            <header class="bs-dialog-header">
                <div>
                    <Dialog.Title class="bs-dialog-title">{title}</Dialog.Title>
                    {#if description}
                        <Dialog.Description class="bs-dialog-description">{description}</Dialog.Description>
                    {/if}
                </div>
                <Dialog.Close class="bs-icon-button" aria-label={t('common.close')}>
                    <XIcon />
                </Dialog.Close>
            </header>

            {#if children}
                <div class="bs-dialog-body">{@render children()}</div>
            {/if}

            {#if footer}
                <footer class="bs-dialog-footer">{@render footer()}</footer>
            {/if}
        </Dialog.Content>
    </Dialog.Portal>
</Dialog.Root>
