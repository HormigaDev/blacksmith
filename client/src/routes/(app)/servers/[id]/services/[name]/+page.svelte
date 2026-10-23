<!--
    Pestaña Resumen (la de entrada al servicio): su consumo y su estado de un vistazo. El estado
    lo refresca el layout del servicio mientras se ve; la tendencia se acumula con cada lectura.
-->
<script lang="ts">
    import { page } from '$app/state';
    import ServiceDashboard from '$lib/components/services/ServiceDashboard.svelte';
    import { servers, serviceStore } from '$lib/stores';

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const name = $derived(page.params.name ?? '');
    const service = $derived(serverId ? serviceStore.service(serverId, name) : null);

    // Rama con la que se construyó la versión activa.
    const branch = $derived(
        service?.runs?.find((run) => run.kind === 'deploy' && run.release === service.status?.release)?.branch ?? null,
    );

    $effect(() => {
        if (service && service.site === undefined) void service.loadSite().catch(() => undefined);
    });
</script>

{#if service}
    <ServiceDashboard status={service.status} history={service.history} {branch} site={service.site} />
{/if}
