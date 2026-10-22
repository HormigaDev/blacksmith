<!--
    Una variante de un componente en el Workbench. Cada `<Story>` es también una unidad de prueba:
    los tests de accesibilidad y visuales (e2e/workbench) la localizan por `data-story`.
-->
<script lang="ts">
    import type { Snippet } from 'svelte';

    import { getWorkbench } from './context';
    import { toSlug } from './stories';

    interface Props {
        name: string;
        description?: string;
        /** `padded` por defecto; `centered` para piezas sueltas; `grid` para comparar variantes. */
        layout?: 'padded' | 'centered' | 'grid' | 'stack';
        children: Snippet;
    }

    let { name, description, layout = 'padded', children }: Props = $props();

    const workbench = getWorkbench();
    const slug = $derived(toSlug(name));
    const visible = $derived(workbench.isolated === null || workbench.isolated === slug);
</script>

{#if visible}
    <section class="story" id={slug} data-story="{workbench.component}--{slug}">
        <header class="header">
            <div>
                <h2 class="name">{name}</h2>
                {#if description}<p class="description">{description}</p>{/if}
            </div>
            {#if workbench.isolated === null}
                <a class="isolate" href="?story={slug}">Aislar</a>
            {/if}
        </header>
        <div class="canvas {layout}" data-story-canvas>
            {@render children()}
        </div>
    </section>
{/if}

<style>
    .story {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .header {
        display: flex;
        align-items: flex-end;
        justify-content: space-between;
        gap: var(--space-4);
    }

    .name {
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-semibold);
    }

    .description {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .isolate {
        font-size: var(--font-size-xs);
        text-decoration: none;
    }

    .canvas {
        padding: var(--space-6);
        background-color: var(--color-bg);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-lg);
    }

    .centered {
        display: flex;
        align-items: center;
        justify-content: center;
        min-height: var(--space-16);
    }

    .grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
        align-items: start;
        gap: var(--space-6) var(--space-4);
    }

    .stack {
        display: flex;
        flex-direction: column;
        align-items: flex-start;
        gap: var(--space-4);
    }

    .padded {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-3);
    }
</style>
