<!-- Contenedor de notificaciones. Va una sola vez, en el layout raíz (ya está). -->
<script lang="ts">
    import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
    import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
    import InfoIcon from '@lucide/svelte/icons/info';
    import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
    import XIcon from '@lucide/svelte/icons/x';

    import { t } from '$lib/i18n';
    import { toast, type ToastVariant } from '$lib/toast';

    const ICONS = {
        info: InfoIcon,
        success: CircleCheckIcon,
        warning: TriangleAlertIcon,
        danger: CircleAlertIcon,
    } satisfies Record<ToastVariant, unknown>;
</script>

<section class="toaster" aria-label={t('components.toast.region')}>
    {#each toast.items as item (item.id)}
        {@const Icon = ICONS[item.variant]}
        <div class="toast {item.variant}" role={item.variant === 'danger' ? 'alert' : 'status'}>
            <span class="icon"><Icon /></span>
            <div class="text">
                <p class="title">{item.title}</p>
                {#if item.description}
                    <p class="description">{item.description}</p>
                {/if}
            </div>
            <button
                type="button"
                class="bs-icon-button"
                aria-label={t('components.toast.dismiss')}
                onclick={() => toast.dismiss(item.id)}
            >
                <XIcon />
            </button>
        </div>
    {/each}
</section>

<style>
    .toaster {
        position: fixed;
        right: var(--space-4);
        bottom: var(--space-4);
        z-index: var(--z-toast);
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        width: min(var(--toast-width), calc(100vw - 2 * var(--space-4)));
        pointer-events: none;
    }

    .toast {
        --_accent: var(--color-info);

        display: flex;
        align-items: flex-start;
        gap: var(--space-3);
        padding: var(--space-3);
        background-color: var(--color-surface-raised);
        border: var(--border-width) solid var(--color-border);
        border-left: var(--border-width-strong) solid var(--_accent);
        border-radius: var(--radius-lg);
        box-shadow: var(--shadow-lg);
        pointer-events: auto;
        animation: slide-in var(--duration-normal) var(--easing-emphasized);
    }

    .success {
        --_accent: var(--color-success);
    }

    .warning {
        --_accent: var(--color-warning);
    }

    .danger {
        --_accent: var(--color-danger);
    }

    .icon {
        display: inline-flex;
        color: var(--_accent);
    }

    .icon :global(svg) {
        width: var(--icon-size-md);
        height: var(--icon-size-md);
    }

    .text {
        display: flex;
        flex: 1;
        flex-direction: column;
        gap: var(--space-px);
        min-width: 0;
    }

    .title {
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-medium);
    }

    .description {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    @keyframes slide-in {
        from {
            opacity: 0;
            transform: translateX(var(--space-4));
        }
    }
</style>
