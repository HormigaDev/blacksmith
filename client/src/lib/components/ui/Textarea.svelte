<!-- Área de texto que crece con el contenido hasta `maxRows`; luego hace scroll. -->
<script lang="ts">
    import type { Snippet } from 'svelte';
    import type { HTMLTextareaAttributes } from 'svelte/elements';

    import { t } from '$lib/i18n';

    import Field from './Field.svelte';

    interface Props extends Omit<HTMLTextareaAttributes, 'value' | 'children'> {
        label: string;
        hideLabel?: boolean;
        hint?: string;
        error?: string;
        value?: string;
        /** Filas visibles mínimas. */
        rows?: number;
        /** Filas máximas antes de hacer scroll. */
        maxRows?: number;
        /** Íconos o acciones en la esquina superior derecha. */
        trailing?: Snippet;
        /** Fuente monoespaciada (llaves, configuración, logs). */
        monospace?: boolean;
    }

    let {
        label,
        hideLabel = false,
        hint,
        error,
        required = false,
        value = $bindable(''),
        rows = 3,
        maxRows = 12,
        trailing,
        monospace = false,
        maxlength,
        disabled,
        id,
        ...rest
    }: Props = $props();

    let textarea = $state<HTMLTextAreaElement>();

    $effect(() => {
        void value;
        if (!textarea) return;
        textarea.style.height = 'auto';
        textarea.style.height = `${textarea.scrollHeight}px`;
    });
</script>

<Field {label} {hideLabel} {hint} {error} required={required ?? false} id={id ?? undefined}>
    {#snippet children({ id: controlId, describedBy, invalid })}
        <div class="bs-control control" data-invalid={invalid || undefined} data-disabled={disabled || undefined}>
            <textarea
                {...rest}
                bind:this={textarea}
                bind:value
                id={controlId}
                class="bs-control-input textarea"
                class:monospace
                {rows}
                {maxlength}
                {required}
                {disabled}
                style:--_max-rows={maxRows}
                aria-invalid={invalid || undefined}
                aria-describedby={describedBy}></textarea>

            {#if trailing}
                <span class="bs-control-icon trailing">{@render trailing()}</span>
            {/if}
        </div>

        {#if maxlength !== undefined && maxlength !== null}
            <span class="counter" class:limit={value.length >= Number(maxlength)} aria-live="polite">
                {t('components.textarea.counter', { count: value.length, max: Number(maxlength) })}
            </span>
        {/if}
    {/snippet}
</Field>

<style>
    .control {
        align-items: flex-start;
        padding-block: var(--space-2);
    }

    .textarea {
        max-height: calc(var(--_max-rows) * 1lh);
        overflow-y: auto;
        resize: none;
        font-family: inherit;
    }

    .monospace {
        font-family: var(--font-mono);
        font-size: var(--font-size-xs);
    }

    .trailing {
        align-self: flex-start;
    }

    .counter {
        align-self: flex-end;
        font-size: var(--font-size-xs);
        font-variant-numeric: tabular-nums;
        color: var(--color-text-subtle);
    }

    .limit {
        color: var(--color-warning);
    }
</style>
