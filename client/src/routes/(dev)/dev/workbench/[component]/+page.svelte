<script lang="ts">
    import { page } from '$app/state';
    import { setWorkbench } from '$lib/workbench';

    import type { PageProps } from './$types';

    let { data }: PageProps = $props();

    setWorkbench({
        get component() {
            return data.slug;
        },
        get isolated() {
            return page.url.searchParams.get('story');
        },
    });

    const Stories = $derived(data.component);
    const isolated = $derived(page.url.searchParams.get('story'));
</script>

<svelte:head>
    <title>{data.meta.title} · Workbench</title>
</svelte:head>

<header class="header">
    <div>
        <p class="group">{data.meta.group}</p>
        <h1 class="title">{data.meta.title}</h1>
        {#if data.meta.description}<p class="description">{data.meta.description}</p>{/if}
    </div>
    {#if isolated}
        <a href="?">Ver todas las variantes</a>
    {/if}
</header>

<div class="stories">
    <Stories />
</div>

<style>
    .header {
        display: flex;
        align-items: flex-end;
        justify-content: space-between;
        gap: var(--space-4);
        margin-bottom: var(--space-8);
    }

    .group {
        font-size: var(--font-size-xs);
        color: var(--color-text-subtle);
    }

    .title {
        font-size: var(--font-size-2xl);
    }

    .description {
        max-width: var(--dialog-width-lg);
        margin-top: var(--space-1);
        color: var(--color-text-muted);
    }

    .stories {
        display: flex;
        flex-direction: column;
        gap: var(--space-10);
    }
</style>
