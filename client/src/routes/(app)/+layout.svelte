<!--
  Grupo (app): pantallas de la aplicación ya configurada (dashboard, servidores…).
  Los flujos a pantalla completa, como la configuración inicial, van en su propio grupo,
  p. ej. src/routes/(setup)/, con su propio +layout.svelte.
-->
<script lang="ts">
    import type { Snippet } from 'svelte';

    import AppShell from '$lib/components/layout/AppShell.svelte';

    let { children }: { children: Snippet } = $props();
</script>

<AppShell>
    {@render children()}
</AppShell>
