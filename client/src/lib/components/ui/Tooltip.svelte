<!--
    Texto de ayuda al pasar el ratón o enfocar. El disparador recibe las props por el snippet:
    <Tooltip text={t('…')}>{#snippet children(props)}<Button {...props}>…</Button>{/snippet}</Tooltip>
    Requiere <Tooltip.Provider> en el layout raíz (ya está).
-->
<script lang="ts">
    import { Tooltip } from 'bits-ui';
    import type { Snippet } from 'svelte';

    interface Props {
        text: string;
        side?: 'top' | 'right' | 'bottom' | 'left';
        children: Snippet<[Record<string, unknown>]>;
    }

    let { text, side = 'top', children }: Props = $props();
</script>

<Tooltip.Root>
    <Tooltip.Trigger>
        {#snippet child({ props })}
            {@render children(props)}
        {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Portal>
        <Tooltip.Content class="bs-tooltip" {side} sideOffset={6} collisionPadding={8}>
            {text}
        </Tooltip.Content>
    </Tooltip.Portal>
</Tooltip.Root>

<style>
    /* Elemento renderizado por bits-ui en un portal: el estilo con alcance no le llega. */
    :global(.bs-tooltip) {
        z-index: var(--z-tooltip);
        max-width: var(--tooltip-max-width);
        padding: var(--space-1) var(--space-2);
        font-size: var(--font-size-xs);
        line-height: var(--line-height-normal);
        color: var(--color-text-inverse);
        background-color: var(--color-text);
        border-radius: var(--radius-sm);
        box-shadow: var(--shadow-md);
        animation: bs-fade-in var(--duration-fast) var(--easing-standard);
    }
</style>
