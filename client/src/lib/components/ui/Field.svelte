<!--
    Envoltorio común de los controles de formulario: etiqueta, ayuda y error, enlazados por
    ARIA. Los controles reciben `id`, `describedBy` e `invalid` a través del snippet.
-->
<script lang="ts" module>
    export interface FieldControl {
        id: string;
        describedBy: string | undefined;
        invalid: boolean;
    }
</script>

<script lang="ts">
    import type { Snippet } from 'svelte';

    import { t } from '$lib/i18n';

    interface Props {
        label: string;
        /** Oculta la etiqueta visualmente; sigue disponible para lectores de pantalla. */
        hideLabel?: boolean;
        hint?: string;
        error?: string;
        required?: boolean;
        /** Etiquetar el control con `aria-labelledby` en vez de `<label for>` (controles que no son input). */
        labelId?: string;
        /** Elemento al que se aplica `for`. Por defecto, el id del control. */
        id?: string;
        children: Snippet<[FieldControl]>;
    }

    let { label, hideLabel = false, hint, error, required = false, labelId, id, children }: Props = $props();

    const generated = $props.id();
    const controlId = $derived(id ?? `field-${generated}`);
    const hintId = $derived(`${controlId}-hint`);
    const errorId = $derived(`${controlId}-error`);
    // Solo se muestra uno de los dos: el error sustituye a la ayuda.
    const describedBy = $derived(error ? errorId : hint ? hintId : undefined);
</script>

<div class="field">
    <label class="label" class:bs-visually-hidden={hideLabel} for={controlId} id={labelId}>
        {label}
        {#if required}
            <span class="required" aria-hidden="true">*</span>
            <span class="bs-visually-hidden">{t('common.required')}</span>
        {/if}
    </label>

    {@render children({ id: controlId, describedBy, invalid: Boolean(error) })}

    {#if error}
        <p class="error" id={errorId}>{error}</p>
    {:else if hint}
        <p class="hint" id={hintId}>{hint}</p>
    {/if}
</div>

<style>
    .field {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
        min-width: 0;
    }

    .label {
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-medium);
        color: var(--color-text);
    }

    .required {
        color: var(--color-danger);
    }

    .hint,
    .error {
        font-size: var(--font-size-xs);
    }

    .hint {
        color: var(--color-text-muted);
    }

    .error {
        color: var(--color-danger);
    }
</style>
