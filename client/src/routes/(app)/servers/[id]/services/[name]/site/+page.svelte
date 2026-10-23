<!--
    Pestaña Web: el sitio del servicio detrás de nginx (plantilla, dominios, puerto y HTTPS). El
    agente genera la configuración, la valida con `nginx -t` y la aplica; el certificado de
    Let's Encrypt llega en segundo plano y el estado se refresca solo mientras tanto.
-->
<script lang="ts">
    import { openUrl } from '@tauri-apps/plugin-opener';

    import { page } from '$app/state';
    import SiteForm from '$lib/components/services/SiteForm.svelte';
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
        if (service && service.site === undefined) {
            service.loadSite().catch((error: unknown) => (loadError = describeError(error)));
        }
    });
</script>

{#if service}
    {#if loadError && service.site === undefined}
        <p class="alert" role="alert">{loadError}</p>
    {:else if service.site === undefined || !service.deployment}
        <Spinner label={t('common.loading')} />
    {:else}
        <Panel title={t('services.site.title')}>
            {#key service.site?.updatedAt ?? 0}
                <SiteForm
                    site={service.site}
                    publishDir={service.deployment.publishDir}
                    agentPort={entry?.tunnel?.remotePort ?? entry?.profile.port}
                    onSave={async (config) => {
                        await service.saveSite(config);
                        toast.success(t('services.site.saved'));
                    }}
                    onRemove={async () => {
                        try {
                            await service.removeSite();
                            toast.success(t('services.site.removed'));
                        } catch (error) {
                            toast.error(describeError(error));
                        }
                    }}
                    onRetryCertificate={() => service.retryCertificate()}
                    onOpenSite={(url) => void openUrl(url)}
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
