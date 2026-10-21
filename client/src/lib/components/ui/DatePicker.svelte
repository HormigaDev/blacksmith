<!--
    Selector de fecha: campo segmentado editable con teclado y calendario desplegable.
    El valor es una fecha ISO (`2026-10-21`); la librería no se filtra al resto de la app.
-->
<script lang="ts">
    import { type DateValue, parseDate } from '@internationalized/date';
    import CalendarIcon from '@lucide/svelte/icons/calendar';
    import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
    import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
    import { DatePicker } from 'bits-ui';

    import { i18n, t } from '$lib/i18n';

    import Field from './Field.svelte';

    interface Props {
        label: string;
        hideLabel?: boolean;
        hint?: string;
        error?: string;
        required?: boolean;
        /** Fecha ISO `AAAA-MM-DD`. */
        value?: string;
        min?: string;
        max?: string;
        disabled?: boolean;
        readonly?: boolean;
        onValueChange?: (value: string | undefined) => void;
    }

    let {
        label,
        hideLabel = false,
        hint,
        error,
        required = false,
        value = $bindable(),
        min,
        max,
        disabled = false,
        readonly = false,
        onValueChange,
    }: Props = $props();

    const uid = $props.id();
    const labelId = `date-${uid}-label`;

    function toDate(iso: string | undefined): DateValue | undefined {
        if (!iso) return undefined;
        try {
            return parseDate(iso);
        } catch {
            return undefined;
        }
    }

    function update(next: DateValue | undefined): void {
        value = next?.toString();
        onValueChange?.(value);
    }
</script>

<Field {label} {hideLabel} {hint} {error} {required} {labelId}>
    {#snippet children({ describedBy, invalid })}
        <DatePicker.Root
            bind:value={() => toDate(value), update}
            locale={i18n.current.intl}
            minValue={toDate(min)}
            maxValue={toDate(max)}
            {disabled}
            {readonly}
            {required}
            weekdayFormat="short"
            fixedWeeks
        >
            <DatePicker.Input
                class="bs-control"
                aria-labelledby={labelId}
                aria-describedby={describedBy}
                data-invalid={invalid || undefined}
            >
                {#snippet children({ segments })}
                    <div class="bs-segments">
                        {#each segments as { part, value: text }, index (index)}
                            <DatePicker.Segment {part} class="bs-segment">{text}</DatePicker.Segment>
                        {/each}
                    </div>
                    <DatePicker.Trigger class="bs-icon-button" aria-label={t('components.datePicker.open')}>
                        <CalendarIcon />
                    </DatePicker.Trigger>
                {/snippet}
            </DatePicker.Input>

            <DatePicker.Portal>
                <DatePicker.Content class="bs-popover bs-calendar-popover" sideOffset={6} collisionPadding={8}>
                    <DatePicker.Calendar class="bs-calendar">
                        {#snippet children({ months, weekdays })}
                            <DatePicker.Header class="bs-calendar-header">
                                <DatePicker.PrevButton
                                    class="bs-icon-button"
                                    aria-label={t('components.datePicker.previousMonth')}
                                >
                                    <ChevronLeftIcon />
                                </DatePicker.PrevButton>
                                <DatePicker.Heading class="bs-calendar-heading" />
                                <DatePicker.NextButton
                                    class="bs-icon-button"
                                    aria-label={t('components.datePicker.nextMonth')}
                                >
                                    <ChevronRightIcon />
                                </DatePicker.NextButton>
                            </DatePicker.Header>

                            {#each months as month (month.value.toString())}
                                <DatePicker.Grid class="bs-calendar-grid">
                                    <DatePicker.GridHead>
                                        <DatePicker.GridRow class="bs-calendar-row">
                                            {#each weekdays as weekday, index (index)}
                                                <DatePicker.HeadCell class="bs-calendar-weekday"
                                                    >{weekday}</DatePicker.HeadCell
                                                >
                                            {/each}
                                        </DatePicker.GridRow>
                                    </DatePicker.GridHead>
                                    <DatePicker.GridBody>
                                        {#each month.weeks as week, index (index)}
                                            <DatePicker.GridRow class="bs-calendar-row">
                                                {#each week as date (date.toString())}
                                                    <DatePicker.Cell
                                                        {date}
                                                        month={month.value}
                                                        class="bs-calendar-cell"
                                                    >
                                                        <DatePicker.Day class="bs-calendar-day" />
                                                    </DatePicker.Cell>
                                                {/each}
                                            </DatePicker.GridRow>
                                        {/each}
                                    </DatePicker.GridBody>
                                </DatePicker.Grid>
                            {/each}
                        {/snippet}
                    </DatePicker.Calendar>
                </DatePicker.Content>
            </DatePicker.Portal>
        </DatePicker.Root>
    {/snippet}
</Field>

<style>
    /* Elementos renderizados por bits-ui (y en un portal): el estilo con alcance no les llega. */
    /* Doble clase: debe ganar a `.bs-popover` sin depender del orden de carga del CSS. */
    :global(.bs-popover.bs-calendar-popover) {
        max-height: none;
        padding: var(--space-3);
        overflow: visible;
    }

    :global(.bs-calendar-header) {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: var(--space-2);
    }

    :global(.bs-calendar-heading) {
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-semibold);
    }

    /* Solo la primera letra: "Octubre de 2026", no "Octubre De 2026". */
    :global(.bs-calendar-heading::first-letter) {
        text-transform: uppercase;
    }

    :global(.bs-calendar-grid) {
        border-collapse: collapse;
    }

    :global(.bs-calendar-weekday) {
        padding: var(--space-1) var(--space-0);
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-medium);
        color: var(--color-text-subtle);
        text-transform: capitalize;
    }

    :global(.bs-calendar-cell) {
        padding: var(--space-px);
    }

    :global(.bs-calendar-day) {
        display: flex;
        align-items: center;
        justify-content: center;
        width: var(--control-height-md);
        height: var(--control-height-md);
        font-size: var(--font-size-sm);
        font-variant-numeric: tabular-nums;
        border-radius: var(--radius-md);
        cursor: pointer;
        transition: background-color var(--duration-fast) var(--easing-standard);
    }

    :global(.bs-calendar-day:hover) {
        background-color: var(--color-surface-hover);
    }

    :global(.bs-calendar-day[data-today]) {
        font-weight: var(--font-weight-bold);
        color: var(--color-accent);
    }

    :global(.bs-calendar-day[data-selected]) {
        font-weight: var(--font-weight-semibold);
        color: var(--color-on-accent);
        background-color: var(--color-accent);
    }

    :global(.bs-calendar-day[data-outside-month]) {
        color: var(--color-text-disabled);
    }

    :global(.bs-calendar-day[data-disabled]) {
        color: var(--color-text-disabled);
        cursor: not-allowed;
    }

    /* Tachado solo lo que no se puede elegir por `min`/`max`, no los días de otro mes. */
    :global(.bs-calendar-day[data-unavailable]),
    :global(.bs-calendar-day[data-disabled]:not([data-outside-month])) {
        text-decoration: line-through;
    }

    :global(.bs-calendar-day:focus-visible) {
        outline: var(--focus-ring-width) solid var(--color-focus-ring);
        outline-offset: var(--space-0);
    }
</style>
