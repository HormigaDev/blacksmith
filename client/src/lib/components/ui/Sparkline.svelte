<!--
    Tendencia de una sola serie (sin ejes): línea de 2 px sobre un área tenue, en el color de
    acento. Un valor `null` deja un hueco (p. ej. el servicio estuvo parado): nunca se dibuja como
    cero.

    Interacción: con el puntero, una línea vertical busca la lectura más cercana y muestra su
    valor y su hora. Con el teclado es un `slider` de lecturas (patrón ARIA): las flechas las
    recorren, Inicio/Fin van a los extremos y Escape cierra el detalle; cada lectura se anuncia
    con `aria-valuetext`. El resumen (ahora, mínimo, máximo) va en la descripción: el gráfico
    nunca es la única forma de leer el dato.
-->
<script lang="ts">
    import { formatDateTime, t } from '$lib/i18n';

    interface Props {
        /** Nombre accesible (p. ej. «CPU en los últimos minutos»). */
        label: string;
        values: readonly (number | null)[];
        /** Hora de cada valor (ms Unix), para el detalle. */
        times?: readonly number[];
        /** Cómo se lee un valor (unidades incluidas). */
        format: (value: number) => string;
        /** Techo del eje vertical; por defecto, el máximo de la serie. */
        max?: number;
    }

    let { label, values, times = [], format, max }: Props = $props();

    const summaryId = $props.id();

    // Sistema de coordenadas fijo; el SVG se estira y el trazo no (non-scaling-stroke).
    const WIDTH = 100;
    const HEIGHT = 32;

    const present = $derived(values.filter((value): value is number => value !== null));
    const top = $derived(Math.max(max ?? 0, ...present, Number.EPSILON));
    const x = (index: number) => (values.length <= 1 ? WIDTH : (index / (values.length - 1)) * WIDTH);
    const y = (value: number) => HEIGHT - (value / top) * (HEIGHT - 2) - 1;

    /** Tramos continuos (sin `null`), como pares [índice, valor]. */
    const segments = $derived.by(() => {
        const result: [number, number][][] = [];
        let current: [number, number][] = [];
        values.forEach((value, index) => {
            if (value === null) {
                if (current.length) result.push(current);
                current = [];
            } else {
                current.push([index, value]);
            }
        });
        if (current.length) result.push(current);
        return result;
    });
    const line = $derived(
        segments
            .map((segment) => segment.map(([index, value], i) => `${i ? 'L' : 'M'}${x(index)},${y(value)}`).join(''))
            .join(''),
    );
    const area = $derived(
        segments
            .filter((segment) => segment.length > 1)
            .map((segment) => {
                const first = segment[0];
                const last = segment[segment.length - 1];
                if (!first || !last) return '';
                const path = segment.map(([index, value]) => `L${x(index)},${y(value)}`).join('');
                return `M${x(first[0])},${HEIGHT}${path}L${x(last[0])},${HEIGHT}Z`;
            })
            .join(''),
    );

    const summary = $derived.by(() => {
        const last = present.at(-1);
        if (last === undefined) return t('components.sparkline.empty', { label });
        return t('components.sparkline.summary', {
            label,
            last: format(last),
            min: format(Math.min(...present)),
            max: format(Math.max(...present)),
        });
    });

    let active = $state<number | null>(null);
    const activeValue = $derived(active === null ? null : (values[active] ?? null));
    const activeTime = $derived(active === null ? undefined : times[active]);
    /** Lectura que anuncia el slider: la activa o, sin ninguna, la última. */
    const announced = $derived(active ?? (values.length ? values.length - 1 : 0));
    const valueText = $derived.by(() => {
        const value = values[announced] ?? null;
        const time = times[announced];
        const reading = value === null ? t('components.sparkline.noData') : format(value);
        return time === undefined ? reading : `${reading} · ${formatDateTime(time, { timeStyle: 'medium' })}`;
    });

    function nearest(event: PointerEvent): void {
        const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
        if (box.width === 0 || values.length === 0) return;
        const ratio = Math.min(1, Math.max(0, (event.clientX - box.left) / box.width));
        active = Math.round(ratio * (values.length - 1));
    }

    function onkeydown(event: KeyboardEvent): void {
        if (values.length === 0) return;
        const last = values.length - 1;
        const moves: Record<string, () => number | null> = {
            ArrowLeft: () => Math.max(0, (active ?? last) - 1),
            ArrowRight: () => Math.min(last, (active ?? last) + 1),
            Home: () => 0,
            End: () => last,
            Escape: () => null,
        };
        const move = moves[event.key];
        if (!move) return;
        event.preventDefault();
        active = move();
    }
</script>

<div
    class="sparkline"
    role="slider"
    aria-label={label}
    aria-describedby={summaryId}
    aria-valuemin={0}
    aria-valuemax={Math.max(0, values.length - 1)}
    aria-valuenow={announced}
    aria-valuetext={valueText}
    tabindex="0"
    onpointermove={nearest}
    onpointerleave={() => (active = null)}
    onblur={() => (active = null)}
    {onkeydown}
>
    <svg viewBox="0 0 {WIDTH} {HEIGHT}" preserveAspectRatio="none" aria-hidden="true">
        {#if area}<path class="area" d={area} />{/if}
        {#if line}<path class="line" d={line} />{/if}
        {#if active !== null}
            <line class="crosshair" x1={x(active)} x2={x(active)} y1="0" y2={HEIGHT} />
        {/if}
    </svg>
    {#if active !== null}
        <!-- Detalle visual del punto; para lectores de pantalla ya lo dice `aria-valuetext`. -->
        <span class="tooltip" class:start={x(active) < WIDTH / 2} style:--position="{x(active)}%" aria-hidden="true">
            <strong>{activeValue === null ? t('components.sparkline.noData') : format(activeValue)}</strong>
            {#if activeTime !== undefined}
                <span class="time">{formatDateTime(activeTime, { timeStyle: 'medium' })}</span>
            {/if}
        </span>
    {/if}
</div>
<span id={summaryId} class="bs-visually-hidden">{summary}</span>

<style>
    .sparkline {
        position: relative;
        width: 100%;
        height: var(--sparkline-height);
        border-radius: var(--radius-sm);
        outline-offset: var(--focus-ring-offset);
    }

    .sparkline:focus-visible {
        outline: var(--focus-ring-width) solid var(--color-focus-ring);
    }

    svg {
        display: block;
        width: 100%;
        height: 100%;
        overflow: visible;
    }

    .line {
        fill: none;
        stroke: var(--color-accent);
        stroke-width: var(--chart-line-width);
        stroke-linejoin: round;
        stroke-linecap: round;
        vector-effect: non-scaling-stroke;
    }

    .area {
        fill: var(--color-accent-subtle);
        stroke: none;
    }

    .crosshair {
        stroke: var(--color-text-muted);
        stroke-width: var(--border-width);
        vector-effect: non-scaling-stroke;
    }

    .tooltip {
        /* Posición horizontal de la lectura (la fija el marcado). */
        --position: 0%;
        position: absolute;
        bottom: calc(100% + var(--space-1));
        left: var(--position);
        z-index: var(--z-tooltip);
        display: flex;
        flex-direction: column;
        padding: var(--space-1) var(--space-2);
        font-size: var(--font-size-xs);
        color: var(--color-text);
        white-space: nowrap;
        pointer-events: none;
        background-color: var(--color-surface-raised);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-sm);
        box-shadow: var(--shadow-md);
        transform: translateX(-100%);
    }

    .tooltip.start {
        transform: none;
    }

    .time {
        color: var(--color-text-muted);
    }
</style>
