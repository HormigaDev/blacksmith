<!--
  Grupo (app): pantallas de la aplicación ya configurada (dashboard, servidores…).
  Los flujos a pantalla completa, como la configuración inicial, van en su propio grupo,
  p. ej. src/routes/(setup)/, con su propio +layout.svelte.
-->
<script lang="ts">
    import type { Snippet } from 'svelte';

    import AppShell from '$lib/components/layout/AppShell.svelte';
    import { toast } from '$lib/toast';
    import { describeError } from '$lib/http';
    import { servers } from '$lib/servers/servers.svelte';

    let { children }: { children: Snippet } = $props();

    // Servidores guardados, y aviso si un túnel se pierde (red caída, servidor reiniciado).
    $effect(() => {
        servers.load().catch((error: unknown) => toast.error(describeError(error)));
    });
    $effect(() => servers.watch());
</script>

<AppShell>
    {@render children()}
</AppShell>
