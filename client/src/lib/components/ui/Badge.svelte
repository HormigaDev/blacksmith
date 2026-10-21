<!-- Etiqueta corta de estado (activo, detenido, fallo…). -->
<script lang="ts">
    import type { Snippet } from 'svelte';

    interface Props {
        variant?: 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info';
        /** Punto de color delante del texto. */
        dot?: boolean;
        children: Snippet;
    }

    let { variant = 'neutral', dot = false, children }: Props = $props();
</script>

<span class="badge {variant}">
    {#if dot}<span class="dot" aria-hidden="true"></span>{/if}
    {@render children()}
</span>

<style>
    .badge {
        --_fg: var(--color-text-muted);
        --_bg: var(--color-bg-subtle);

        display: inline-flex;
        align-items: center;
        gap: var(--space-1);
        padding: var(--space-px) var(--space-2);
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-medium);
        line-height: var(--line-height-normal);
        white-space: nowrap;
        color: var(--_fg);
        background-color: var(--_bg);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-full);
    }

    .accent {
        --_fg: var(--color-accent);
        --_bg: var(--color-accent-subtle);
    }

    .success {
        --_fg: var(--color-success);
        --_bg: var(--color-success-subtle);
    }

    .warning {
        --_fg: var(--color-warning);
        --_bg: var(--color-warning-subtle);
    }

    .danger {
        --_fg: var(--color-danger);
        --_bg: var(--color-danger-subtle);
    }

    .info {
        --_fg: var(--color-info);
        --_bg: var(--color-info-subtle);
    }

    .accent,
    .success,
    .warning,
    .danger,
    .info {
        border-color: transparent;
    }

    .dot {
        width: var(--space-2);
        height: var(--space-2);
        background-color: currentColor;
        border-radius: var(--radius-full);
    }
</style>
