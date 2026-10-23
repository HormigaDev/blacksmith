<!--
    Un servidor configurado: estado de su conexión y acciones (conectar, reintentar, quitar…).

    Con `href`, toda la tarjeta lleva a la página del servidor: el título es el enlace y su
    `::after` cubre la tarjeta (enlace estirado). Los botones y el asa quedan por encima, así que
    no hay controles anidados dentro de un enlace.
-->
<script lang="ts">
    import ServerIcon from '@lucide/svelte/icons/server';
    import Trash2Icon from '@lucide/svelte/icons/trash-2';
    import type { Snippet } from 'svelte';

    import { t } from '$lib/i18n';
    import { serverAddress } from '$lib/servers/address';
    import type { ServerEntry } from '$lib/servers/servers.svelte';

    import Badge from '../ui/Badge.svelte';
    import Button from '../ui/Button.svelte';
    import ConfirmDialog from '../ui/ConfirmDialog.svelte';
    import Tooltip from '../ui/Tooltip.svelte';
    import ServerStatusBadge from './ServerStatusBadge.svelte';

    interface Props {
        entry: ServerEntry;
        onConnect: () => void | Promise<void>;
        onDisconnect: () => void | Promise<void>;
        onRemove: () => void | Promise<void>;
        onForgetHostKey: () => void | Promise<void>;
        /** `row` en la vista de lista; `tile` en la cuadrícula. */
        layout?: 'row' | 'tile';
        /** Asa para reordenar (la pone `ServerList`). */
        handle?: Snippet;
        /** Página del servidor: hace la tarjeta entera clicable. */
        href?: string;
    }

    let { entry, onConnect, onDisconnect, onRemove, onForgetHostKey, layout = 'row', handle, href }: Props = $props();

    let removeOpen = $state(false);
    let forgetOpen = $state(false);

    const profile = $derived(entry.profile);
    const isLocal = $derived(profile.kind === 'local');
    const address = $derived(serverAddress(profile));
    const busy = $derived(entry.status === 'connecting');
    const retry = $derived(entry.status === 'agentMissing' || entry.status === 'error');

    const detail = $derived.by(() => {
        if (entry.status === 'connected' && entry.tunnel) {
            const port = entry.tunnel.localPort;
            return isLocal
                ? t('servers.card.connectedLocal', { port })
                : t('servers.connect.connectedDetail', { port });
        }
        if (entry.status === 'agentMissing') {
            return isLocal
                ? t('servers.card.agentMissingLocalHint', { port: profile.port })
                : t('servers.card.agentMissingHint');
        }
        if (entry.status === 'error') return entry.error;
        return undefined;
    });
</script>

<article class="card {layout}" class:linked={href} data-status={entry.status} aria-busy={busy || undefined}>
    {#if handle}<div class="handle">{@render handle()}</div>{/if}
    <span class="icon"><ServerIcon /></span>

    <div class="body">
        <div class="heading">
            <h3 class="address">
                {#if href}<a class="link" {href}>{address}</a>{:else}{address}{/if}
            </h3>
            <ServerStatusBadge status={entry.status} />
            {#if isLocal}<Badge variant="accent">{t('servers.card.local')}</Badge>{/if}
            {#if entry.status === 'connected' && entry.tunnel?.agent}
                <span class="version">{t('servers.card.agentVersion', { version: entry.tunnel.agent.version })}</span>
            {/if}
        </div>
        {#if detail}
            <p class="detail" class:problem={entry.status === 'error' || entry.status === 'agentMissing'}>
                {detail}
            </p>
        {/if}
    </div>

    <div class="actions">
        {#if entry.errorCode === 'HOST_KEY_CHANGED'}
            <Button size="sm" variant="danger" onclick={() => (forgetOpen = true)}>
                {t('servers.connect.changed.forget')}
            </Button>
        {/if}

        {#if entry.status === 'connected'}
            <Button size="sm" onclick={onDisconnect}>{t('servers.connect.disconnect')}</Button>
        {:else}
            <Button size="sm" variant="primary" loading={busy} onclick={onConnect}>
                {retry ? t('servers.card.retry') : t('servers.card.connect')}
            </Button>
        {/if}

        <Tooltip text={t('servers.remove.action')}>
            {#snippet children(props)}
                <Button
                    {...props}
                    size="sm"
                    variant="ghost"
                    iconOnly
                    aria-label={t('servers.remove.label', { address })}
                    disabled={busy}
                    onclick={() => (removeOpen = true)}
                >
                    <Trash2Icon />
                </Button>
            {/snippet}
        </Tooltip>
    </div>
</article>

<ConfirmDialog
    bind:open={removeOpen}
    variant="danger"
    title={t('servers.remove.title', { address })}
    description={t('servers.remove.description')}
    confirmLabel={t('servers.remove.confirm')}
    onConfirm={onRemove}
/>

<ConfirmDialog
    bind:open={forgetOpen}
    variant="danger"
    title={t('servers.connect.changed.title', { host: profile.host })}
    description={t('servers.connect.changed.description')}
    confirmLabel={t('servers.connect.changed.confirm')}
    onConfirm={onForgetHostKey}
/>

<style>
    .card {
        position: relative;
        display: flex;
        align-items: center;
        gap: var(--space-4);
        padding: var(--space-4) var(--space-5);
        background-color: var(--color-surface);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-lg);
    }

    /* Enlace estirado: el área del enlace es la tarjeta entera. */
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

    .link:focus-visible {
        outline: none;
    }

    /* Los controles quedan por encima del enlace. */
    .handle,
    .actions {
        position: relative;
        z-index: var(--z-raised);
    }

    /* Cuadrícula: apilado vertical, acciones al pie. */
    .tile {
        flex-direction: column;
        align-items: stretch;
        height: 100%;
    }

    .tile .icon {
        align-self: flex-start;
    }

    .tile .body {
        flex: 1;
    }

    .tile .actions {
        justify-content: flex-end;
        padding-top: var(--space-3);
        border-top: var(--border-width) solid var(--color-border);
    }

    .handle {
        display: inline-flex;
        flex-shrink: 0;
    }

    .tile .handle {
        position: absolute;
        top: var(--space-2);
        right: var(--space-2);
    }

    .icon {
        display: inline-flex;
        color: var(--color-text-subtle);
    }

    .icon :global(svg) {
        width: var(--icon-size-lg);
        height: var(--icon-size-lg);
    }

    .body {
        display: flex;
        flex: 1;
        flex-direction: column;
        gap: var(--space-1);
        min-width: 0;
    }

    .heading {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-2);
    }

    .address {
        overflow: hidden;
        font-family: var(--font-mono);
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-semibold);
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .version {
        font-family: var(--font-mono);
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .detail {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .detail.problem {
        color: var(--color-text);
    }

    .actions {
        display: flex;
        flex-shrink: 0;
        align-items: center;
        gap: var(--space-2);
    }
</style>
