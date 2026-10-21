<!-- Grupo segmentado de selección única (p. ej. vista lista/cuadrícula, rango de tiempo). -->
<script lang="ts" generics="T extends string">
    import { ToggleGroup } from 'bits-ui';
    import type { Component } from 'svelte';

    interface Option {
        value: T;
        label: string;
        icon?: Component;
        disabled?: boolean;
    }

    interface Props {
        /** Nombre accesible del grupo. */
        label: string;
        options: readonly Option[];
        value?: T;
        /** Solo íconos; `label` de cada opción pasa a ser su nombre accesible. */
        iconOnly?: boolean;
        size?: 'sm' | 'md';
        disabled?: boolean;
        onValueChange?: (value: T) => void;
    }

    let {
        label,
        options,
        value = $bindable(),
        iconOnly = false,
        size = 'md',
        disabled = false,
        onValueChange,
    }: Props = $props();

    // Siempre hay una opción activa: pulsar la seleccionada no la desmarca.
    function select(next: string): void {
        const option = options.find((candidate) => candidate.value === next);
        if (!option) return;
        value = option.value;
        onValueChange?.(option.value);
    }
</script>

<ToggleGroup.Root
    type="single"
    bind:value={() => value ?? '', select}
    aria-label={label}
    {disabled}
    class="bs-toggle-group {size === 'sm' ? 'bs-toggle-group-sm' : ''}"
>
    {#each options as option (option.value)}
        {@const Icon = option.icon}
        <ToggleGroup.Item
            value={option.value}
            disabled={option.disabled}
            aria-label={iconOnly ? option.label : undefined}
            title={iconOnly ? option.label : undefined}
            class="bs-toggle"
        >
            {#if Icon}<Icon />{/if}
            {#if !iconOnly}<span>{option.label}</span>{/if}
        </ToggleGroup.Item>
    {/each}
</ToggleGroup.Root>
