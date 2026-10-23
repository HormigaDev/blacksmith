<!-- Pestaña Detalles del servidor: sistema, hardware y uso, refrescados mientras se ven. -->
<script lang="ts">
    import { page } from '$app/state';
    import HostOverview from '$lib/components/servers/HostOverview.svelte';
    import { Panel, Spinner } from '$lib/components/ui';
    import { t } from '$lib/i18n';
    import { servers, serviceStore } from '$lib/stores';

    /** Cada cuánto se refrescan los datos a la vista (memoria, discos, carga). */
    const HOST_INTERVAL_MS = 10_000;

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const server = $derived(serverId ? serviceStore.server(serverId) : null);

    // Cada lectura programa la siguiente al terminar: con un servidor lento, nunca se solapan
    // (una respuesta vieja no pisa a una nueva).
    $effect(() => {
        if (!server || entry?.status !== 'connected') return;
        let stopped = false;
        let timer: ReturnType<typeof setTimeout> | undefined;
        const tick = async () => {
            await server.loadHost();
            if (!stopped) timer = setTimeout(tick, HOST_INTERVAL_MS);
        };
        void tick();
        return () => {
            stopped = true;
            clearTimeout(timer);
        };
    });
</script>

{#if server}
    <Panel title={t('host.title')}>
        {#if server.host}
            <HostOverview host={server.host} />
        {:else if server.hostError}
            <p class="alert" role="alert">{server.hostError}</p>
        {:else}
            <Spinner label={t('host.loading')} />
        {/if}
    </Panel>
{/if}

<style>
    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }
</style>
