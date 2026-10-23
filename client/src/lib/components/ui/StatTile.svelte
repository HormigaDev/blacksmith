<!--
    Indicador de un vistazo: etiqueta, valor y detalle opcional, con un hueco para su tendencia o
    su medidor (snippet `children`). El valor va en la tipografía normal, con cifras
    proporcionales.
-->
<script lang="ts">
    import type { Snippet } from 'svelte';

    interface Props {
        label: string;
        /** Ya formateado (con unidades). */
        value: string;
        /** Contexto del valor (p. ej. «de 12 núcleos», «pico 1,2 GB»). */
        detail?: string;
        /** `text`: valores de texto (fechas, direcciones) en un tamaño menor que las cifras. */
        kind?: 'number' | 'text';
        children?: Snippet;
    }

    let { label, value, detail, kind = 'number', children }: Props = $props();
</script>

<section class="tile" aria-label={label}>
    <h3 class="label">{label}</h3>
    <p class="value" class:text={kind === 'text'} title={kind === 'text' ? value : undefined}>{value}</p>
    {#if detail}<p class="detail">{detail}</p>{/if}
    {#if children}<div class="extra">{@render children()}</div>{/if}
</section>

<style>
    .tile {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
        min-width: 0;
        padding: var(--space-4);
        background-color: var(--color-surface);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-lg);
    }

    .label {
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-medium);
        color: var(--color-text-muted);
    }

    .value {
        font-size: var(--font-size-2xl);
        font-weight: var(--font-weight-semibold);
        line-height: var(--line-height-tight);
        letter-spacing: var(--letter-spacing-tight);
        color: var(--color-text);
        overflow-wrap: anywhere;
    }

    /* Una línea: una dirección partida a mitad no se lee; el valor entero va en `title`. */
    .value.text {
        overflow: hidden;
        font-size: var(--font-size-md);
        font-weight: var(--font-weight-medium);
        letter-spacing: normal;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .detail {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .extra {
        margin-top: var(--space-2);
    }
</style>
