<!-- Interruptor de encendido/apagado para ajustes que se aplican de inmediato. -->
<script lang="ts">
    import { Switch } from 'bits-ui';

    interface Props {
        label: string;
        description?: string;
        checked?: boolean;
        disabled?: boolean;
        required?: boolean;
        name?: string;
        onCheckedChange?: (checked: boolean) => void;
    }

    let {
        label,
        description,
        checked = $bindable(false),
        disabled = false,
        required = false,
        name,
        onCheckedChange,
    }: Props = $props();

    const uid = $props.id();
    const id = `switch-${uid}`;
</script>

<div class="switch" class:disabled>
    <div class="text">
        <label class="label" for={id}>{label}</label>
        {#if description}
            <p class="description" id="{id}-description">{description}</p>
        {/if}
    </div>

    <Switch.Root
        {id}
        bind:checked
        {disabled}
        {required}
        {name}
        {onCheckedChange}
        aria-describedby={description ? `${id}-description` : undefined}
        class="bs-switch"
    >
        <Switch.Thumb class="bs-switch-thumb" />
    </Switch.Root>
</div>

<style>
    .switch {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
    }

    .text {
        display: flex;
        flex-direction: column;
        gap: var(--space-px);
    }

    .label {
        font-size: var(--font-size-sm);
        cursor: pointer;
    }

    .description {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .disabled .label {
        color: var(--color-text-disabled);
        cursor: not-allowed;
    }

    /* Elementos renderizados por bits-ui: el estilo con alcance no les llega. */
    :global(.bs-switch) {
        display: inline-flex;
        flex-shrink: 0;
        align-items: center;
        width: var(--control-height-md);
        height: var(--icon-size-md);
        padding: var(--space-px);
        background-color: var(--color-border-strong);
        border: var(--border-width) solid transparent;
        border-radius: var(--radius-full);
        transition: background-color var(--duration-fast) var(--easing-standard);
    }

    :global(.bs-switch[data-state='checked']) {
        background-color: var(--color-accent);
    }

    :global(.bs-switch:disabled) {
        opacity: 0.5;
        cursor: not-allowed;
    }

    :global(.bs-switch-thumb) {
        display: block;
        width: calc(var(--icon-size-md) - 4 * var(--space-px));
        height: calc(var(--icon-size-md) - 4 * var(--space-px));
        background-color: var(--color-surface);
        border-radius: var(--radius-full);
        box-shadow: var(--shadow-sm);
        transition: transform var(--duration-normal) var(--easing-emphasized);
    }

    :global(.bs-switch-thumb[data-state='checked']) {
        transform: translateX(calc(var(--control-height-md) - var(--icon-size-md)));
    }
</style>
