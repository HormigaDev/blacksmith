<script lang="ts">
    interface Props {
        size?: 'sm' | 'md' | 'lg';
        /** Si se indica, se anuncia a lectores de pantalla; si no, es decorativo. */
        label?: string;
    }

    let { size = 'md', label }: Props = $props();
</script>

<span
    class="spinner {size}"
    role={label ? 'status' : undefined}
    aria-label={label}
    aria-hidden={label ? undefined : 'true'}
></span>

<style>
    .spinner {
        display: inline-block;
        flex-shrink: 0;
        width: var(--icon-size-md);
        height: var(--icon-size-md);
        border: var(--border-width-strong) solid currentColor;
        border-right-color: transparent;
        border-radius: var(--radius-full);
        animation: spin var(--duration-spin) var(--easing-linear) infinite;
    }

    .sm {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
    }

    .lg {
        width: var(--icon-size-lg);
        height: var(--icon-size-lg);
    }

    @keyframes spin {
        to {
            transform: rotate(1turn);
        }
    }
</style>
