<!--
    Página de un servidor: por ahora, la lista (vacía) de sus servicios. Se llega desde la tarjeta
    del servidor en el dashboard.
-->
<script lang="ts">
    import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
    import ServerCogIcon from '@lucide/svelte/icons/server-cog';

    import { page } from '$app/state';
    import { APP_NAME } from '$lib/app';
    import ServerStatusBadge from '$lib/components/servers/ServerStatusBadge.svelte';
    import { Badge } from '$lib/components/ui';
    import { t } from '$lib/i18n';
    import { serverAddress } from '$lib/servers/address';
    import { servers } from '$lib/servers/servers.svelte';

    const entry = $derived(servers.find(page.params.id ?? ''));
    const address = $derived(entry ? serverAddress(entry.profile) : '');
</script>

<svelte:head>
    <title>{entry ? address : t('servers.detail.notFound.title')} · {APP_NAME}</title>
</svelte:head>

<div class="page">
    <a class="back" href="/">
        <ArrowLeftIcon />
        {t('servers.detail.back')}
    </a>

    {#if entry}
        <header class="header">
            <h1 class="title">{address}</h1>
            <ServerStatusBadge status={entry.status} />
            {#if entry.profile.kind === 'local'}<Badge variant="accent">{t('servers.card.local')}</Badge>{/if}
        </header>

        <section class="services" aria-labelledby="services-title">
            <h2 id="services-title" class="section-title">{t('servers.detail.services.title')}</h2>
            <div class="empty">
                <span class="empty-icon"><ServerCogIcon /></span>
                <p class="empty-title">{t('servers.detail.services.emptyTitle')}</p>
                <p class="empty-description">
                    {entry.status === 'connected'
                        ? t('servers.detail.services.emptyDescription')
                        : t('servers.detail.services.notConnected')}
                </p>
            </div>
        </section>
    {:else if servers.loaded}
        <section class="empty">
            <h1 class="empty-title">{t('servers.detail.notFound.title')}</h1>
            <p class="empty-description">{t('servers.detail.notFound.description')}</p>
        </section>
    {/if}
</div>

<style>
    .page {
        display: flex;
        flex-direction: column;
        gap: var(--space-5);
    }

    .back {
        display: inline-flex;
        align-self: flex-start;
        align-items: center;
        gap: var(--space-1);
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
        text-decoration: none;
        border-radius: var(--radius-sm);
        transition: color var(--duration-fast) var(--easing-standard);
    }

    .back:hover {
        color: var(--color-text);
    }

    .back:focus-visible {
        outline: var(--focus-ring-width) solid var(--color-focus-ring);
        outline-offset: var(--focus-ring-width);
    }

    .back :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
    }

    .header {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-3);
    }

    .title {
        font-family: var(--font-mono);
        font-size: var(--font-size-xl);
    }

    .services {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .section-title {
        font-size: var(--font-size-lg);
    }

    .empty {
        display: grid;
        justify-items: center;
        gap: var(--space-2);
        padding: var(--space-8) var(--space-4);
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

    .empty-description {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }
</style>
