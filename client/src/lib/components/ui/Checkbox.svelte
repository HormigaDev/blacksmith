<script lang="ts">
    import CheckIcon from '@lucide/svelte/icons/check';
    import MinusIcon from '@lucide/svelte/icons/minus';
    import { Checkbox } from 'bits-ui';

    interface Props {
        label: string;
        description?: string;
        checked?: boolean;
        /** Estado mixto (p. ej. "seleccionar todo" con una selección parcial). */
        indeterminate?: boolean;
        disabled?: boolean;
        required?: boolean;
        name?: string;
        value?: string;
        onCheckedChange?: (checked: boolean) => void;
    }

    let {
        label,
        description,
        checked = $bindable(false),
        indeterminate = $bindable(false),
        disabled = false,
        required = false,
        name,
        value,
        onCheckedChange,
    }: Props = $props();

    const uid = $props.id();
    const id = `checkbox-${uid}`;
</script>

<div class="checkbox" class:disabled>
    <Checkbox.Root
        {id}
        bind:checked
        bind:indeterminate
        {disabled}
        {required}
        {name}
        {value}
        {onCheckedChange}
        aria-describedby={description ? `${id}-description` : undefined}
        class="bs-checkbox"
    >
        {#snippet children({ checked: isChecked, indeterminate: isIndeterminate })}
            {#if isIndeterminate}
                <MinusIcon />
            {:else if isChecked}
                <CheckIcon />
            {/if}
        {/snippet}
    </Checkbox.Root>

    <div class="text">
        <label class="label" for={id}>{label}</label>
        {#if description}
            <p class="description" id="{id}-description">{description}</p>
        {/if}
    </div>
</div>

<style>
    .checkbox {
        display: flex;
        align-items: flex-start;
        gap: var(--space-2);
    }

    .text {
        display: flex;
        flex-direction: column;
        gap: var(--space-px);
    }

    .label {
        font-size: var(--font-size-sm);
        line-height: var(--line-height-tight);
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

    /* Elemento renderizado por bits-ui: el estilo con alcance no le llega. */
    :global(.bs-checkbox) {
        display: inline-flex;
        flex-shrink: 0;
        align-items: center;
        justify-content: center;
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
        padding: var(--space-0);
        color: var(--color-on-accent);
        background-color: var(--color-surface);
        border: var(--border-width) solid var(--color-border-strong);
        border-radius: var(--radius-sm);
        transition:
            background-color var(--duration-fast) var(--easing-standard),
            border-color var(--duration-fast) var(--easing-standard);
    }

    :global(.bs-checkbox:hover:not(:disabled)) {
        border-color: var(--color-accent);
    }

    :global(.bs-checkbox[data-state='checked']),
    :global(.bs-checkbox[data-state='indeterminate']) {
        background-color: var(--color-accent);
        border-color: var(--color-accent);
    }

    :global(.bs-checkbox:disabled) {
        background-color: var(--color-bg-subtle);
        border-color: var(--color-border);
        cursor: not-allowed;
    }

    :global(.bs-checkbox svg) {
        width: var(--font-size-sm);
        height: var(--font-size-sm);
        stroke-width: 3;
    }
</style>
