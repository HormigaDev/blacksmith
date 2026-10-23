<!-- Pestaña Servicios del servidor: los registrados, registrar uno nuevo y quitarlos. -->
<script lang="ts">
    import PlusIcon from '@lucide/svelte/icons/plus';
    import ServerCogIcon from '@lucide/svelte/icons/server-cog';

    import { page } from '$app/state';
    import type { RegisterService, ServiceSummary } from '$lib/agent';
    import RegisterServiceForm from '$lib/components/services/RegisterServiceForm.svelte';
    import ServiceCard from '$lib/components/services/ServiceCard.svelte';
    import { Button, Dialog, Spinner } from '$lib/components/ui';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';
    import { serviceHref } from '$lib/servers/address';
    import { servers, serviceStore } from '$lib/stores';
    import { toast } from '$lib/toast';

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const services = $derived(serverId ? serviceStore.server(serverId) : null);
    const titleId = $props.id();

    let registerOpen = $state(false);

    $effect(() => {
        if (services && entry?.status === 'connected') void services.load();
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

{#if serverId && services}
    <section class="services" aria-labelledby={titleId}>
        <header class="header">
            <h2 class="title" id={titleId}>{t('servers.detail.services.title')}</h2>
            <Button variant="primary" onclick={() => (registerOpen = true)}>
                <PlusIcon />
                {t('services.register.open')}
            </Button>
        </header>

        {#if services.error && services.list === null}
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
            <ul class="grid" aria-labelledby={titleId}>
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
    </section>

    <Dialog bind:open={registerOpen} title={t('services.register.title')} size="lg">
        <RegisterServiceForm onSubmit={register} />
    </Dialog>
{/if}

<style>
    .services {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
    }

    .header {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-3);
    }

    .title {
        font-size: var(--font-size-lg);
        font-weight: var(--font-weight-semibold);
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

    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }
</style>
