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
    import { AGENT_PORT } from '$lib/ssh';
    import { servers, serviceStore } from '$lib/stores';
    import { toast } from '$lib/toast';

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const name = $derived(page.params.name ?? '');
    const service = $derived(serverId ? serviceStore.service(serverId, name) : null);
    let loadError = $state<string | null>(null);
    // El formulario se vuelve a montar solo al guardar o quitar el sitio (para mostrar lo que
    // quedó). No con cada cambio del sitio: mientras llega el certificado se refresca solo y se
    // perdería lo que el usuario está escribiendo.
    let saves = $state(0);

    $effect(() => {
        if (service && service.site === undefined) {
            loadError = null;
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
            {#key saves}
                <SiteForm
                    site={service.site}
                    publishDir={service.deployment.publishDir}
                    agentPort={entry?.tunnel?.remotePort ?? AGENT_PORT}
                    onSave={async (config) => {
                        await service.saveSite(config);
                        saves++;
                        toast.success(t('services.site.saved'));
                    }}
                    onRemove={async () => {
                        try {
                            await service.removeSite();
                        } catch (error) {
                            toast.error(describeError(error));
                            // El diálogo de confirmación sigue abierto: no se quitó nada.
                            throw error;
                        }
                        saves++;
                        toast.success(t('services.site.removed'));
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
