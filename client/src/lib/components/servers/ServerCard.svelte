<!-- Un servidor configurado: estado de su conexión y acciones (conectar, reintentar, quitar…). -->
<script lang="ts">
    import ServerIcon from '@lucide/svelte/icons/server';
    import Trash2Icon from '@lucide/svelte/icons/trash-2';

    import { t } from '$lib/i18n';
    import type { ServerEntry } from '$lib/servers/servers.svelte';

    import Badge from '../ui/Badge.svelte';
    import Button from '../ui/Button.svelte';
    import ConfirmDialog from '../ui/ConfirmDialog.svelte';
    import Tooltip from '../ui/Tooltip.svelte';

    interface Props {
        entry: ServerEntry;
        onConnect: () => void | Promise<void>;
        onDisconnect: () => void | Promise<void>;
        onRemove: () => void | Promise<void>;
        onForgetHostKey: () => void | Promise<void>;
    }

    let { entry, onConnect, onDisconnect, onRemove, onForgetHostKey }: Props = $props();

    let removeOpen = $state(false);
    let forgetOpen = $state(false);

    const profile = $derived(entry.profile);
    const address = $derived(`${profile.username}@${profile.host}${profile.port === 22 ? '' : `:${profile.port}`}`);
    const busy = $derived(entry.status === 'connecting');
    const retry = $derived(entry.status === 'agentMissing' || entry.status === 'error');

    const badge = $derived({
        disconnected: { variant: 'neutral', label: t('servers.status.disconnected') },
        connecting: { variant: 'info', label: t('servers.status.connecting') },
        connected: { variant: 'success', label: t('servers.status.connected') },
        agentMissing: { variant: 'warning', label: t('servers.status.agentMissing') },
        error: { variant: 'danger', label: t('servers.status.error') },
    } as const satisfies Record<ServerEntry['status'], { variant: string; label: string }>);
    const status = $derived(badge[entry.status]);

    const detail = $derived.by(() => {
        if (entry.status === 'connected' && entry.tunnel) {
            return t('servers.connect.connectedDetail', { port: entry.tunnel.localPort });
        }
        if (entry.status === 'agentMissing') return t('servers.card.agentMissingHint');
        if (entry.status === 'error') return entry.error;
        return undefined;
    });
</script>

<article class="card" data-status={entry.status} aria-busy={busy || undefined}>
    <span class="icon"><ServerIcon /></span>

    <div class="body">
        <div class="heading">
            <h3 class="address">{address}</h3>
            <Badge variant={status.variant} dot>{status.label}</Badge>
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
        display: flex;
        align-items: center;
        gap: var(--space-4);
        padding: var(--space-4) var(--space-5);
        background-color: var(--color-surface);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-lg);
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
