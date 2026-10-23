<!--
    Pestaña Recursos: límites de CPU, memoria, swap y procesos del servicio. Se aplican al
    momento (systemd los pone en el cgroup de la unidad) y el consumo actual sirve de referencia.
-->
<script lang="ts">
    import { page } from '$app/state';
    import ResourceLimitsForm from '$lib/components/services/ResourceLimitsForm.svelte';
    import { Panel, Spinner } from '$lib/components/ui';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';
    import { servers, serviceStore } from '$lib/stores';
    import { toast } from '$lib/toast';

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const name = $derived(page.params.name ?? '');
    const service = $derived(serverId ? serviceStore.service(serverId, name) : null);
    let loadError = $state<string | null>(null);

    $effect(() => {
        if (service && service.limits === null) {
            loadError = null;
            service.loadLimits().catch((error: unknown) => (loadError = describeError(error)));
        }
    });
</script>

{#if service}
    {#if loadError && service.limits === null}
        <p class="alert" role="alert">{loadError}</p>
    {:else if service.limits === null}
        <Spinner label={t('common.loading')} />
    {:else}
        <Panel title={t('services.limits.title')}>
            {#key service.limits.updatedAt ?? 0}
                <ResourceLimitsForm
                    limits={service.limits}
                    usage={service.status?.resources ?? null}
                    onSave={async (limits) => {
                        await service.saveLimits(limits);
                        toast.success(t('services.limits.saved'));
                    }}
                />
            {/key}
        </Panel>
    {/if}
{/if}

<style>
    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }
</style>
