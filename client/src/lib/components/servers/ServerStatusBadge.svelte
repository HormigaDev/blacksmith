<!-- Estado de la conexión con un servidor, como insignia. -->
<script lang="ts">
    import { t } from '$lib/i18n';
    import type { ServerEntry } from '$lib/servers/servers.svelte';

    import Badge from '../ui/Badge.svelte';

    interface Props {
        status: ServerEntry['status'];
    }

    let { status }: Props = $props();

    const badges = $derived({
        disconnected: { variant: 'neutral', label: t('servers.status.disconnected') },
        connecting: { variant: 'info', label: t('servers.status.connecting') },
        connected: { variant: 'success', label: t('servers.status.connected') },
        agentMissing: { variant: 'warning', label: t('servers.status.agentMissing') },
        error: { variant: 'danger', label: t('servers.status.error') },
    } as const satisfies Record<ServerEntry['status'], { variant: string; label: string }>);
    const badge = $derived(badges[status]);
</script>

<Badge variant={badge.variant} dot>{badge.label}</Badge>
