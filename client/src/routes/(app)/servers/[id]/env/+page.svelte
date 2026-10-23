<!-- Variables globales del servidor: comunes a todos sus servicios (las de un servicio ganan). -->
<script lang="ts">
    import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';

    import { page } from '$app/state';
    import { APP_NAME } from '$lib/app';
    import EnvEditor from '$lib/components/services/EnvEditor.svelte';
    import { Panel, Spinner } from '$lib/components/ui';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';
    import { serverAddress, serverHref } from '$lib/servers/address';
    import { servers, serviceStore } from '$lib/stores';
    import { toast } from '$lib/toast';

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const connected = $derived(entry?.status === 'connected');
    const server = $derived(serverId ? serviceStore.server(serverId) : null);

    let loadError = $state<string | null>(null);

    $effect(() => {
        if (!server || !connected) return;
        loadError = null;
        server.loadGlobal().catch((error: unknown) => (loadError = describeError(error)));
    });
</script>

<svelte:head>
    <title>{t('services.env.globalTitle')} · {APP_NAME}</title>
</svelte:head>

{#if entry && serverId && server}
    <div class="page">
        <header class="header">
            <nav class="breadcrumb" aria-label={t('services.breadcrumb')}>
                <a href={serverHref(serverId)}>{serverAddress(entry.profile)}</a>
                <ChevronRightIcon />
            </nav>
            <h1 class="title">{t('services.env.globalTitle')}</h1>
            <p class="note">{t('services.env.globalDescription')}</p>
        </header>
        {#if !connected}
            <p class="note">{t('servers.detail.services.notConnected')}</p>
        {:else if loadError}
            <p class="alert" role="alert">{loadError}</p>
        {:else if server.global}
            <Panel>
                <EnvEditor
                    variables={server.global.variables}
                    onSave={async (variables) => {
                        await server.saveGlobal(variables);
                        toast.success(t('services.env.saved'));
                    }}
                />
            </Panel>
        {:else}
            <Spinner label={t('common.loading')} />
        {/if}
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
        flex-direction: column;
        gap: var(--space-1);
    }

    .breadcrumb {
        display: flex;
        align-items: center;
        gap: var(--space-1);
        font-family: var(--font-mono);
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .breadcrumb a {
        color: inherit;
        text-decoration: none;
    }

    .breadcrumb :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
    }

    .title {
        font-size: var(--font-size-2xl);
        letter-spacing: var(--letter-spacing-tight);
    }

    .note {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }
</style>
