<!-- Página de un servidor: sus servicios registrados. -->
<script lang="ts">
    import PlusIcon from '@lucide/svelte/icons/plus';
    import ServerCogIcon from '@lucide/svelte/icons/server-cog';

    import { page } from '$app/state';
    import type { RegisterService, ServiceSummary } from '$lib/agent';
    import { APP_NAME } from '$lib/app';
    import ServerStatusBadge from '$lib/components/servers/ServerStatusBadge.svelte';
    import RegisterServiceForm from '$lib/components/services/RegisterServiceForm.svelte';
    import ServiceCard from '$lib/components/services/ServiceCard.svelte';
    import { Badge, Button, Dialog, Spinner } from '$lib/components/ui';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';
    import { serverAddress, serverHref, serviceHref } from '$lib/servers/address';
    import { servers, serviceStore } from '$lib/stores';
    import { toast } from '$lib/toast';

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const connected = $derived(entry?.status === 'connected');
    const services = $derived(serverId ? serviceStore.server(serverId) : null);

    let registerOpen = $state(false);

    $effect(() => {
        if (services && connected) void services.load();
    });

    async function register(service: RegisterService): Promise<void> {
        if (!services) return;
        const created = await services.register(service);
        registerOpen = false;
        toast.success(t('services.register.done', { name: created.displayName }));
    }

    async function remove(service: ServiceSummary): Promise<void> {
        try {
            await services?.remove(service.name);
            toast.success(t('services.remove.done', { name: service.displayName }));
        } catch (error) {
            toast.error(describeError(error));
        }
    }
</script>

<svelte:head>
    <title>{entry ? serverAddress(entry.profile) : t('servers.detail.notFound.title')} · {APP_NAME}</title>
</svelte:head>

{#if entry && serverId && services}
    <div class="page">
        <header class="header">
            <div class="heading">
                <h1 class="title">{serverAddress(entry.profile)}</h1>
                <ServerStatusBadge status={entry.status} />
                {#if entry.profile.kind === 'local'}<Badge variant="accent">{t('servers.card.local')}</Badge>{/if}
            </div>
            {#if connected}
                <div class="actions">
                    <a class="link" href="{serverHref(serverId)}/env">{t('services.env.globalLink')}</a>
                    <Button variant="primary" onclick={() => (registerOpen = true)}>
                        <PlusIcon />
                        {t('services.register.open')}
                    </Button>
                </div>
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
        {:else if services.error && services.list === null}
            <div class="empty">
                <p class="alert" role="alert">{services.error}</p>
                <Button onclick={() => services.load()}>{t('services.list.retry')}</Button>
            </div>
        {:else if services.list === null}
            <div class="empty"><Spinner label={t('services.list.loading')} /></div>
        {:else if services.list.length === 0}
            <div class="empty">
                <span class="empty-icon"><ServerCogIcon /></span>
                <p class="empty-title">{t('servers.detail.services.emptyTitle')}</p>
            </div>
        {:else}
            <ul class="grid" aria-label={t('servers.detail.services.title')}>
                {#each services.list as service (service.name)}
                    <li>
                        <ServiceCard
                            {service}
                            href={serviceHref(serverId, service.name)}
                            onRemove={() => remove(service)}
                        />
                    </li>
                {/each}
            </ul>
        {/if}
    </div>

    <Dialog bind:open={registerOpen} title={t('services.register.title')} size="lg">
        <RegisterServiceForm onSubmit={register} />
    </Dialog>
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
        gap: var(--space-6);
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

    .actions {
        display: flex;
        align-items: center;
        gap: var(--space-4);
    }

    .link {
        font-size: var(--font-size-sm);
        color: var(--color-accent);
    }

    .grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(min(100%, var(--layout-card-min-width)), 1fr));
        gap: var(--space-4);
        padding: var(--space-0);
        margin: var(--space-0);
        list-style: none;
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

    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }
</style>
