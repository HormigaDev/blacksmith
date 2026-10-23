<!-- Sección de una página: superficie elevada con título opcional y acciones a la derecha. -->
<script lang="ts">
    import type { Snippet } from 'svelte';

    interface Props {
        title?: string;
        /** Acciones de la sección (botones, enlaces), alineadas a la derecha del título. */
        actions?: Snippet;
        children: Snippet;
    }

    let { title, actions, children }: Props = $props();
    const id = $props.id();
</script>

<section class="panel" aria-labelledby={title ? `${id}-title` : undefined}>
    {#if title || actions}
        <header class="header">
            {#if title}<h2 id="{id}-title" class="title">{title}</h2>{/if}
            {#if actions}<div class="actions">{@render actions()}</div>{/if}
        </header>
    {/if}
    {@render children()}
</section>

<style>
    .panel {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
        padding: var(--space-5) var(--space-6);
        background-color: var(--color-surface);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-lg);
        box-shadow: var(--shadow-sm);
    }

    .header {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-3);
    }

    .title {
        font-size: var(--font-size-md);
        font-weight: var(--font-weight-semibold);
        letter-spacing: var(--letter-spacing-tight);
    }

    .actions {
        display: flex;
        align-items: center;
        gap: var(--space-2);
    }
</style>
