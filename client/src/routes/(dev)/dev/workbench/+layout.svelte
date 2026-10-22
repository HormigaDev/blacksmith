<!--
    Workbench: catálogo aislado de componentes, solo en desarrollo.
    `pnpm workbench` (ventana de Tauri) · `pnpm workbench:web` (navegador). Ver client/docs/testing.md.
-->
<script lang="ts">
    import SearchIcon from '@lucide/svelte/icons/search';
    import type { Snippet } from 'svelte';

    import { page } from '$app/state';
    import LocaleSwitcher from '$lib/components/locale/LocaleSwitcher.svelte';
    import ThemeSwitcher from '$lib/components/theme/ThemeSwitcher.svelte';
    import { Input } from '$lib/components/ui';
    import { GROUP_ORDER, STORIES, type StoryMeta } from '$lib/workbench';

    let { children }: { children: Snippet } = $props();

    let query = $state('');

    // Las historias se cargan bajo demanda; el menú necesita sus metadatos.
    const metas = Promise.all(STORIES.map(async (story) => ({ ...story, meta: (await story.load()).meta })));

    function grouped(entries: Awaited<typeof metas>, filter: string) {
        const needle = filter.trim().toLowerCase();
        const groups = new Map<string, (typeof entries)[number][]>();
        for (const entry of entries) {
            if (needle && !`${entry.meta.title} ${entry.meta.group}`.toLowerCase().includes(needle)) continue;
            groups.set(entry.meta.group, [...(groups.get(entry.meta.group) ?? []), entry]);
        }
        const rank = (group: string) => (GROUP_ORDER.includes(group) ? GROUP_ORDER.indexOf(group) : GROUP_ORDER.length);
        return [...groups].sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b));
    }

    const current = $derived(page.params.component);
    const title = (meta: StoryMeta) => meta.title;
</script>

<svelte:head>
    <title>Workbench</title>
</svelte:head>

<div class="workbench">
    <aside class="sidebar">
        <a class="brand" href="/dev/workbench">Workbench</a>
        <Input label="Buscar componente" hideLabel type="search" size="sm" placeholder="Buscar…" bind:value={query}>
            {#snippet leading()}<SearchIcon />{/snippet}
        </Input>
        <nav class="nav" aria-label="Componentes">
            {#await metas then entries}
                {#each grouped(entries, query) as [group, items] (group)}
                    <p class="group">{group}</p>
                    <ul class="items">
                        {#each items as item (item.slug)}
                            <li>
                                <a
                                    class="item"
                                    href="/dev/workbench/{item.slug}"
                                    aria-current={current === item.slug ? 'page' : undefined}
                                    data-component={item.slug}>{title(item.meta)}</a
                                >
                            </li>
                        {/each}
                    </ul>
                {/each}
            {/await}
        </nav>
    </aside>

    <div class="main">
        <header class="toolbar">
            <LocaleSwitcher />
            <ThemeSwitcher />
        </header>
        <main class="content">
            {@render children()}
        </main>
    </div>
</div>

<style>
    .workbench {
        display: grid;
        grid-template-columns: var(--layout-sidebar-width) minmax(0, 1fr);
        /* Fila de alto fijo: sin ella crece con el contenido y hace scroll la página entera. El
           menú queda fijo y solo `.content` (y el propio menú, si es largo) hace scroll. */
        grid-template-rows: minmax(0, 1fr);
        height: 100vh;
        overflow: hidden;
    }

    .sidebar {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
        padding: var(--space-4);
        overflow-y: auto;
        background-color: var(--color-bg-subtle);
        border-right: var(--border-width) solid var(--color-border);
    }

    .brand {
        font-weight: var(--font-weight-bold);
        color: var(--color-text);
        text-decoration: none;
    }

    .nav {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
    }

    .group {
        margin-top: var(--space-3);
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-semibold);
        color: var(--color-text-subtle);
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }

    .items {
        display: flex;
        flex-direction: column;
        gap: var(--space-px);
        padding: var(--space-0);
        list-style: none;
    }

    .item {
        display: block;
        padding: var(--space-1) var(--space-2);
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
        text-decoration: none;
        border-radius: var(--radius-sm);
    }

    .item:hover {
        color: var(--color-text);
        background-color: var(--color-surface-hover);
    }

    .item[aria-current='page'] {
        color: var(--color-accent);
        background-color: var(--color-accent-subtle);
        font-weight: var(--font-weight-medium);
    }

    .main {
        display: grid;
        grid-template-rows: var(--layout-header-height) minmax(0, 1fr);
        min-width: 0;
    }

    .toolbar {
        display: flex;
        align-items: center;
        justify-content: flex-end;
        gap: var(--space-3);
        padding: var(--space-0) var(--space-6);
        border-bottom: var(--border-width) solid var(--color-border);
    }

    /* Bloque contenedor de los elementos absolutos (etiquetas ocultas, controles flotantes):
       sin él escapan del recorte y alargan el documento, que vuelve a hacer scroll entero. */
    .content {
        position: relative;
        overflow-y: auto;
        padding: var(--space-8);
    }
</style>
