<!--
    Un servicio registrado en el agente. Con `href`, toda la tarjeta lleva a su página (enlace
    estirado, como `ServerCard`); quitarlo del registro se confirma antes.
-->
<script lang="ts">
    import CogIcon from '@lucide/svelte/icons/cog';
    import Trash2Icon from '@lucide/svelte/icons/trash-2';

    import type { ServiceSummary } from '$lib/agent';
    import { t } from '$lib/i18n';

    import Button from '../ui/Button.svelte';
    import ConfirmDialog from '../ui/ConfirmDialog.svelte';
    import Tooltip from '../ui/Tooltip.svelte';

    interface Props {
        service: ServiceSummary;
        href?: string;
        onRemove: () => void | Promise<void>;
    }

    let { service, href, onRemove }: Props = $props();

    let removeOpen = $state(false);
</script>

<article class="card" class:linked={href}>
    <div class="heading">
        <span class="icon"><CogIcon /></span>
        <h3 class="title">
            {#if href}<a class="link" {href}>{service.displayName}</a>{:else}{service.displayName}{/if}
        </h3>
        <div class="actions">
            <Tooltip text={t('services.remove.action')}>
                {#snippet children(props)}
                    <Button
                        {...props}
                        size="sm"
                        variant="ghost"
                        iconOnly
                        aria-label={t('services.remove.label', { name: service.displayName })}
                        onclick={() => (removeOpen = true)}
                    >
                        <Trash2Icon />
                    </Button>
                {/snippet}
            </Tooltip>
        </div>
    </div>
    <p class="unit">{service.unit}</p>
    {#if service.description}<p class="description">{service.description}</p>{/if}
</article>

<ConfirmDialog
    bind:open={removeOpen}
    variant="danger"
    title={t('services.remove.title', { name: service.displayName })}
    description={t('services.remove.description')}
    confirmLabel={t('services.remove.confirm')}
    onConfirm={onRemove}
/>

<style>
    .card {
        position: relative;
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        padding: var(--space-4) var(--space-5);
        background-color: var(--color-surface);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-lg);
    }

    .linked {
        transition: border-color var(--duration-fast) var(--easing-standard);
    }

    .linked:hover {
        border-color: var(--color-border-strong);
    }

    .linked:has(.link:focus-visible) {
        outline: var(--focus-ring-width) solid var(--color-focus-ring);
        outline-offset: var(--focus-ring-width);
    }

    .heading {
        display: flex;
        align-items: center;
        gap: var(--space-3);
    }

    .icon {
        display: inline-flex;
        color: var(--color-text-subtle);
    }

    .icon :global(svg) {
        width: var(--icon-size-md);
        height: var(--icon-size-md);
    }

    .title {
        flex: 1;
        min-width: 0;
        overflow: hidden;
        font-size: var(--font-size-md);
        font-weight: var(--font-weight-semibold);
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    /* Enlace estirado: el área del enlace es la tarjeta entera; las acciones quedan encima. */
    .link {
        color: inherit;
        text-decoration: none;
    }

    .link::after {
        position: absolute;
        inset: 0;
        border-radius: inherit;
        content: '';
    }

    .link:focus-visible {
        outline: none;
    }

    .actions {
        position: relative;
        z-index: var(--z-raised);
    }

    .unit {
        font-family: var(--font-mono);
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .description {
        display: -webkit-box;
        overflow: hidden;
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
        white-space: pre-line;
        -webkit-box-orient: vertical;
        -webkit-line-clamp: 3;
        line-clamp: 3;
    }
</style>
