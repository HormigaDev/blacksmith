<!--
    Proporción de un valor frente a un límite (memoria frente a su límite, disco…). La barra pasa
    de acento a aviso (≥ 75 %) y a peligro (≥ 90 %), y el estado también se dice con texto: nunca
    solo con el color. La pista es un tono claro del mismo color que la barra.
-->
<script lang="ts">
    import { formatPercent, t } from '$lib/i18n';

    interface Props {
        /** Nombre accesible (p. ej. «Memoria frente a la RAM del servidor»). */
        label: string;
        value: number;
        max: number;
        /** Texto del valor para lectores de pantalla (p. ej. «512 MB de 2 GB»). */
        valueText?: string;
    }

    let { label, value, max, valueText }: Props = $props();

    const ratio = $derived(max > 0 ? Math.min(1, Math.max(0, value / max)) : 0);
    const level = $derived(ratio >= 0.9 ? 'danger' : ratio >= 0.75 ? 'warning' : 'normal');
    const levelText = $derived(
        level === 'danger' ? t('components.meter.critical') : level === 'warning' ? t('components.meter.high') : null,
    );
</script>

<div class="meter">
    <div
        class="track {level}"
        role="meter"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={max}
        aria-valuenow={Math.min(value, max)}
        aria-valuetext={[valueText ?? formatPercent(ratio), levelText].filter(Boolean).join(' · ')}
    >
        <span class="fill" style:--ratio={ratio}></span>
    </div>
    <span class="caption">
        {formatPercent(ratio, 0)}
        {#if levelText}<span class="level {level}">· {levelText}</span>{/if}
    </span>
</div>

<style>
    .meter {
        display: flex;
        align-items: center;
        gap: var(--space-2);
    }

    .track {
        flex: 1;
        height: var(--meter-height);
        overflow: hidden;
        background-color: var(--color-accent-subtle);
        border-radius: var(--radius-full);
    }

    .track.warning {
        background-color: var(--color-warning-subtle);
    }

    .track.danger {
        background-color: var(--color-danger-subtle);
    }

    .fill {
        /* Proporción 0-1 (la fija el marcado). */
        --ratio: 0;
        display: block;
        width: calc(var(--ratio) * 100%);
        height: 100%;
        background-color: var(--color-accent);
        border-radius: var(--radius-full);
        transition: width var(--duration-normal) var(--easing-standard);
    }

    .warning .fill {
        background-color: var(--color-warning);
    }

    .danger .fill {
        background-color: var(--color-danger);
    }

    .caption {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
        white-space: nowrap;
    }

    .level.warning,
    .level.danger {
        font-weight: var(--font-weight-medium);
        color: var(--color-text);
    }
</style>
