<!--
    Lista desplegable con búsqueda, para listas largas (repositorios, ramas). Se ve como `Select`:
    el disparador muestra el valor elegido. Al abrirse, un campo filtra mientras se escribe (sin
    distinguir mayúsculas, por cualquier parte del texto); flechas y Enter eligen. Al elegir, o al
    cerrar, la búsqueda se vacía: el valor nunca aparece dos veces.
-->
<script lang="ts" generics="T extends string">
    import CheckIcon from '@lucide/svelte/icons/check';
    import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
    import SearchIcon from '@lucide/svelte/icons/search';
    import { Command, Popover } from 'bits-ui';
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
        /** Texto del campo de búsqueda. */
        searchPlaceholder?: string;
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
        searchPlaceholder,
        disabled = false,
        size = 'md',
        leading,
        onValueChange,
    }: Props = $props();

    let open = $state(false);
    let search = $state('');

    const selected = $derived(options.find((option) => option.value === value));

    /** Coincide si el texto buscado está en la etiqueta o el valor (1) o no (0). */
    function filter(itemValue: string, query: string, keywords?: string[]): number {
        const needle = query.trim().toLowerCase();
        if (!needle) return 1;
        return [itemValue, ...(keywords ?? [])].some((text) => text.toLowerCase().includes(needle)) ? 1 : 0;
    }

    function choose(option: Option): void {
        if (option.disabled) return;
        value = option.value;
        onValueChange?.(option.value);
        open = false;
        search = '';
    }
</script>

<Field {label} {hideLabel} {hint} {error} {required}>
    {#snippet children({ id, describedBy, invalid })}
        <Popover.Root
            bind:open
            onOpenChange={(next) => {
                if (!next) search = '';
            }}
        >
            <Popover.Trigger
                {id}
                {disabled}
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
            </Popover.Trigger>

            <Popover.Portal>
                <Popover.Content
                    class="bs-popover bs-filter-popover"
                    sideOffset={4}
                    collisionPadding={8}
                    align="start"
                    aria-label={label}
                >
                    <Command.Root
                        label={t('components.filterSelect.searchLabel', { label })}
                        {filter}
                        value={value ?? ''}
                        loop
                    >
                        <div class="bs-filter-search">
                            <SearchIcon aria-hidden="true" />
                            <Command.Input
                                class="bs-filter-input"
                                placeholder={searchPlaceholder ?? t('components.filterSelect.search')}
                                autocomplete="off"
                                spellcheck="false"
                                bind:value={search}
                            />
                        </div>
                        <Command.List class="bs-filter-list" aria-label={label}>
                            <Command.Viewport>
                                <Command.Empty class="bs-filter-empty"
                                    >{t('components.filterSelect.empty')}</Command.Empty
                                >
                                {#each options as option (option.value)}
                                    <Command.Item
                                        value={option.value}
                                        keywords={[option.label]}
                                        disabled={option.disabled}
                                        class="bs-option bs-filter-option"
                                        data-chosen={option.value === value ? '' : undefined}
                                        onSelect={() => choose(option)}
                                    >
                                        <span class="bs-option-label">{option.label}</span>
                                        <span class="bs-option-check">
                                            {#if option.value === value}
                                                <CheckIcon aria-hidden="true" />
                                                <span class="bs-visually-hidden"
                                                    >{t('components.filterSelect.chosen')}</span
                                                >
                                            {/if}
                                        </span>
                                    </Command.Item>
                                {/each}
                            </Command.Viewport>
                        </Command.List>
                    </Command.Root>
                </Popover.Content>
            </Popover.Portal>
        </Popover.Root>
    {/snippet}
</Field>
