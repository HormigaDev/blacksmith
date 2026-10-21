<!-- Lista desplegable accesible (teclado, typeahead, ARIA) con el aspecto de la app. -->
<script lang="ts" generics="T extends string">
    import CheckIcon from '@lucide/svelte/icons/check';
    import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
    import { Select } from 'bits-ui';
    import type { Snippet } from 'svelte';

    import { t } from '$lib/i18n';

    import Field from './Field.svelte';

    interface Option {
        value: T;
        label: string;
        disabled?: boolean;
    }

    interface Props {
        label: string;
        hideLabel?: boolean;
        hint?: string;
        error?: string;
        required?: boolean;
        options: readonly Option[];
        value?: T;
        placeholder?: string;
        disabled?: boolean;
        size?: 'sm' | 'md' | 'lg';
        leading?: Snippet;
        onValueChange?: (value: T) => void;
    }

    let {
        label,
        hideLabel = false,
        hint,
        error,
        required = false,
        options,
        value = $bindable(),
        placeholder,
        disabled = false,
        size = 'md',
        leading,
        onValueChange,
    }: Props = $props();

    const selected = $derived(options.find((option) => option.value === value));
    const items = $derived(options.map((option) => ({ ...option })));

    function select(next: string): void {
        const option = options.find((candidate) => candidate.value === next);
        if (!option) return;
        value = option.value;
        onValueChange?.(option.value);
    }
</script>

<Field {label} {hideLabel} {hint} {error} {required}>
    {#snippet children({ id, describedBy, invalid })}
        <Select.Root type="single" bind:value={() => value ?? '', select} {items} {disabled} {required}>
            <Select.Trigger
                {id}
                class="bs-control bs-select-trigger {size === 'md' ? '' : `bs-control-${size}`}"
                aria-describedby={describedBy}
                aria-invalid={invalid || undefined}
                data-invalid={invalid || undefined}
            >
                {#if leading}
                    <span class="bs-control-icon">{@render leading()}</span>
                {/if}
                <span class="bs-select-value" data-placeholder={selected ? undefined : ''}>
                    {selected?.label ?? placeholder ?? t('components.select.placeholder')}
                </span>
                <ChevronDownIcon class="bs-select-chevron" />
            </Select.Trigger>

            <Select.Portal>
                <Select.Content class="bs-popover" sideOffset={4} collisionPadding={8}>
                    {#each options as option (option.value)}
                        <Select.Item
                            value={option.value}
                            label={option.label}
                            disabled={option.disabled}
                            class="bs-option"
                        >
                            {#snippet children({ selected: isSelected })}
                                <span class="bs-option-label">{option.label}</span>
                                <span class="bs-option-check">
                                    {#if isSelected}<CheckIcon />{/if}
                                </span>
                            {/snippet}
                        </Select.Item>
                    {/each}
                </Select.Content>
            </Select.Portal>
        </Select.Root>
    {/snippet}
</Field>
