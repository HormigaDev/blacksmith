<!-- Botón con estado activado/desactivado (p. ej. "seguir logs en vivo"). -->
<script lang="ts">
    import { Toggle } from 'bits-ui';
    import type { Snippet } from 'svelte';

    interface Props {
        pressed?: boolean;
        disabled?: boolean;
        size?: 'sm' | 'md';
        /** Nombre accesible; obligatorio si solo hay un ícono. */
        'aria-label'?: string;
        onPressedChange?: (pressed: boolean) => void;
        children: Snippet;
    }

    let {
        pressed = $bindable(false),
        disabled = false,
        size = 'md',
        onPressedChange,
        children,
        ...rest
    }: Props = $props();
</script>

<Toggle.Root
    bind:pressed
    {disabled}
    {onPressedChange}
    aria-label={rest['aria-label']}
    class="bs-toggle bs-toggle-standalone {size === 'sm' ? 'bs-toggle-sm' : ''}"
>
    {@render children()}
</Toggle.Root>
