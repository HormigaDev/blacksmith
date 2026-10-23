<!--
    Marco de la página de un servidor: cabecera (dirección, estado y acciones) y dos pestañas,
    Detalles (sistema, hardware y uso) y Servicios. Sin conexión, en lugar de las pestañas se pide
    conectar: ninguna de las dos tiene datos sin el agente.
-->
<script lang="ts">
    import ServerCogIcon from '@lucide/svelte/icons/server-cog';
    import type { Snippet } from 'svelte';

    import { page } from '$app/state';
    import { APP_NAME } from '$lib/app';
    import ServerStatusBadge from '$lib/components/servers/ServerStatusBadge.svelte';
    import { Badge, Button, TabNav } from '$lib/components/ui';
    import { t } from '$lib/i18n';
    import { serverAddress, serverHref, serverServicesHref } from '$lib/servers/address';
    import { servers } from '$lib/stores';

    let { children }: { children: Snippet } = $props();

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const connected = $derived(entry?.status === 'connected');

    const tabs = $derived.by(() => {
        if (!serverId) return [];
        const details = serverHref(serverId);
        const services = serverServicesHref(serverId);
        return [
            { href: details, label: t('servers.detail.tabs.details'), current: page.url.pathname === details },
            { href: services, label: t('servers.detail.tabs.services'), current: page.url.pathname === services },
        ];
    });
</script>

<svelte:head>
    <title>{entry ? serverAddress(entry.profile) : t('servers.detail.notFound.title')} · {APP_NAME}</title>
</svelte:head>

{#if entry && serverId}
    <div class="page">
        <header class="header">
            <div class="heading">
                <h1 class="title">{serverAddress(entry.profile)}</h1>
                <ServerStatusBadge status={entry.status} />
                {#if entry.profile.kind === 'local'}<Badge variant="accent">{t('servers.card.local')}</Badge>{/if}
            </div>
            {#if connected}
                <a class="link" href="{serverHref(serverId)}/env">{t('services.env.globalLink')}</a>
            {/if}
        </header>

        {#if !connected}
            <div class="empty">
                <span class="empty-icon"><ServerCogIcon /></span>
                <p class="empty-text">{entry.error ?? t('servers.detail.services.notConnected')}</p>
                <Button
                    variant="primary"
                    loading={entry.status === 'connecting'}
                    onclick={() => servers.connect(serverId)}
                >
                    {entry.status === 'error' ? t('servers.card.retry') : t('servers.card.connect')}
                </Button>
            </div>
        {:else}
            <TabNav label={t('servers.detail.tabs.label')} items={tabs} />
            {@render children()}
        {/if}
    </div>
{:else if servers.loaded}
    <div class="empty">
        <h1 class="empty-title">{t('servers.detail.notFound.title')}</h1>
        <p class="empty-text">{t('servers.detail.notFound.description')}</p>
    </div>
{/if}

<style>
    .page {
        display: flex;
        flex-direction: column;
        gap: var(--space-5);
    }

    .header {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
    }

    .heading {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-3);
    }

    .title {
        font-family: var(--font-mono);
        font-size: var(--font-size-2xl);
        letter-spacing: var(--letter-spacing-tight);
    }

    .link {
        font-size: var(--font-size-sm);
        color: var(--color-accent);
    }

    .empty {
        display: grid;
        justify-items: center;
        gap: var(--space-3);
        padding: var(--space-12) var(--space-4);
        text-align: center;
        border: var(--border-width) dashed var(--color-border-strong);
        border-radius: var(--radius-lg);
    }

    .empty-icon {
        display: inline-flex;
        color: var(--color-text-subtle);
    }

    .empty-icon :global(svg) {
        width: var(--icon-size-lg);
        height: var(--icon-size-lg);
    }

    .empty-title {
        font-size: var(--font-size-md);
        font-weight: var(--font-weight-semibold);
    }

    .empty-text {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }
</style>
