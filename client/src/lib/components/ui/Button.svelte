<script lang="ts">
    import type { Snippet } from 'svelte';
    import type { HTMLButtonAttributes } from 'svelte/elements';

    import { t } from '$lib/i18n';

    import Spinner from './Spinner.svelte';

    interface Props extends Omit<HTMLButtonAttributes, 'children'> {
        variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
        size?: 'sm' | 'md' | 'lg';
        /** Botón cuadrado de solo ícono. Requiere `aria-label`. */
        iconOnly?: boolean;
        loading?: boolean;
        fullWidth?: boolean;
        children: Snippet;
    }

    let {
        variant = 'secondary',
        size = 'md',
        iconOnly = false,
        loading = false,
        fullWidth = false,
        // `button` por defecto: un botón dentro de un <form> no debe enviarlo por accidente.
        type = 'button',
        disabled,
        children,
        ...rest
    }: Props = $props();
</script>

<button
    {...rest}
    {type}
    class="button {variant} {size}"
    class:icon-only={iconOnly}
    class:full-width={fullWidth}
    disabled={disabled || loading}
    aria-busy={loading || undefined}
>
    {#if loading}
        <Spinner size="sm" />
        <span class="bs-visually-hidden">{t('common.loading')}</span>
    {/if}
    <span class="content" class:hidden={loading && iconOnly}>{@render children()}</span>
</button>

<style>
    .button {
        --_bg: var(--color-surface);
        --_bg-hover: var(--color-surface-hover);
        --_bg-active: var(--color-surface-active);
        --_fg: var(--color-text);
        --_border: var(--color-border);

        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: var(--space-2);
        height: var(--control-height-md);
        padding: var(--space-0) var(--space-4);
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-medium);
        line-height: var(--line-height-tight);
        white-space: nowrap;
        color: var(--_fg);
        background-color: var(--_bg);
        border: var(--border-width) solid var(--_border);
        border-radius: var(--radius-md);
        transition:
            background-color var(--duration-fast) var(--easing-standard),
            border-color var(--duration-fast) var(--easing-standard),
            color var(--duration-fast) var(--easing-standard);
    }

    .button:hover:not(:disabled) {
        background-color: var(--_bg-hover);
    }

    .button:active:not(:disabled) {
        background-color: var(--_bg-active);
    }

    .button:disabled {
        opacity: 0.55;
    }

    .primary {
        --_bg: var(--color-accent);
        --_bg-hover: var(--color-accent-hover);
        --_bg-active: var(--color-accent-active);
        --_fg: var(--color-on-accent);
        --_border: var(--color-accent);
    }

    .danger {
        --_bg: var(--color-danger);
        --_bg-hover: var(--color-danger);
        --_bg-active: var(--color-danger);
        --_fg: var(--color-on-danger);
        --_border: var(--color-danger);
    }

    .danger:hover:not(:disabled) {
        filter: brightness(1.08);
    }

    .ghost {
        --_bg: transparent;
        --_border: transparent;
    }

    .sm {
        height: var(--control-height-sm);
        padding: var(--space-0) var(--space-3);
        font-size: var(--font-size-xs);
    }

    .lg {
        height: var(--control-height-lg);
        padding: var(--space-0) var(--space-6);
        font-size: var(--font-size-md);
    }

    .icon-only {
        aspect-ratio: 1;
        padding: var(--space-0);
    }

    .full-width {
        width: 100%;
    }

    .content {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
    }

    .content :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
    }

    .hidden {
        display: none;
    }
</style>
