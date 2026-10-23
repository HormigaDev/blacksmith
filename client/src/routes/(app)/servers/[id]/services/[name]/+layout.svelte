<!--
    Marco de la página de un servicio: ruta de navegación, controles (estado y acciones) y pestañas
    (Logs, Despliegue, Variables). Los datos viven en `serviceStore`: las pestañas comparten la
    misma copia y el estado se refresca solo mientras se ve.
-->
<script lang="ts">
    import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
    import type { Snippet } from 'svelte';

    import { page } from '$app/state';
    import type { ServiceAction } from '$lib/agent';
    import { APP_NAME } from '$lib/app';
    import ServiceControls from '$lib/components/services/ServiceControls.svelte';
    import { TabNav } from '$lib/components/ui';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';
    import { serverAddress, serverHref, serviceHref } from '$lib/servers/address';
    import { servers, serviceStore } from '$lib/stores';
    import { toast } from '$lib/toast';

    let { children }: { children: Snippet } = $props();

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const connected = $derived(entry?.status === 'connected');
    const name = $derived(page.params.name ?? '');
    const server = $derived(serverId ? serviceStore.server(serverId) : null);
    const service = $derived(serverId ? serviceStore.service(serverId, name) : null);
    const summary = $derived(server?.find(name));

    const title = $derived(summary?.displayName ?? name);
    const base = $derived(serverId ? serviceHref(serverId, name) : '');
    const tabs = $derived([
        { href: base, label: t('services.tabs.overview'), current: page.url.pathname === base },
        { href: `${base}/logs`, label: t('services.tabs.logs'), current: page.url.pathname === `${base}/logs` },
        {
            href: `${base}/deployment`,
            label: t('services.tabs.deployment'),
            current: page.url.pathname === `${base}/deployment`,
        },
        { href: `${base}/site`, label: t('services.tabs.site'), current: page.url.pathname === `${base}/site` },
        { href: `${base}/env`, label: t('services.tabs.env'), current: page.url.pathname === `${base}/env` },
    ]);

    $effect(() => {
        if (!server || !service || !connected) return;
        if (server.list === null) void server.load();
        void service.load();
        return service.watch();
    });

    async function act(action: ServiceAction): Promise<void> {
        try {
            await service?.act(action);
            toast.success(t(`services.controls.done.${action}`, { name: title }));
        } catch (error) {
            toast.error(describeError(error));
        }
    }
</script>

<svelte:head>
    <title>{title} · {APP_NAME}</title>
</svelte:head>

{#if entry && serverId && service}
    <div class="page">
        <header class="header">
            <nav class="breadcrumb" aria-label={t('services.breadcrumb')}>
                <a href={serverHref(serverId)}>{serverAddress(entry.profile)}</a>
                <ChevronRightIcon />
                <span aria-current="page">{name}</span>
            </nav>
            <h1 class="title">{title}</h1>
            {#if summary?.description}<p class="description">{summary.description}</p>{/if}
        </header>

        {#if !connected}
            <p class="note">{t('services.detail.notConnected')}</p>
        {:else if server?.list && !summary}
            <p class="alert" role="alert">{t('services.detail.notFound')}</p>
        {:else}
            <ServiceControls
                name={title}
                status={service.status}
                busy={service.busy}
                staticOnly={service.deployment !== null &&
                    service.deployment.startCommand === '' &&
                    service.deployment.publishDir !== null}
                onAction={act}
            />
            <TabNav label={t('services.tabs.label')} items={tabs} />
            {@render children()}
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

    .breadcrumb a:hover {
        color: var(--color-text);
    }

    .breadcrumb :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
    }

    .title {
        font-size: var(--font-size-2xl);
        letter-spacing: var(--letter-spacing-tight);
    }

    .description {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
        white-space: pre-line;
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
