<!-- Pestaña Logs: los logs del servicio en tiempo real. La sesión se cierra al salir. -->
<script lang="ts">
    import { page } from '$app/state';
    import { followLogs } from '$lib/agent';
    import LogViewer from '$lib/components/services/LogViewer.svelte';
    import { servers } from '$lib/servers/servers.svelte';
    import { LogSession } from '$lib/services/log-session.svelte';

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const connected = $derived(entry?.status === 'connected');
    const name = $derived(page.params.name ?? '');

    let session = $state<LogSession>();

    $effect(() => {
        const id = serverId;
        const current = name;
        if (!id || !connected) return;
        const logs = new LogSession((onEvent) => followLogs(id, current, onEvent));
        session = logs;
        void logs.start();
        return () => void logs.stop();
    });
</script>

{#if session}
    <LogViewer
        {name}
        lines={session.lines}
        status={session.status}
        endReason={session.endReason}
        error={session.error}
        onReconnect={() => session?.start()}
    />
{/if}
