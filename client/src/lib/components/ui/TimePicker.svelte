<!--
    Selector de hora segmentado: se edita con teclado (dígitos y flechas). El valor es una hora
    ISO (`14:30` o `14:30:15`); el formato visible (12/24 h) sigue al idioma activo.
-->
<script lang="ts">
    import { Time } from '@internationalized/date';
    import ClockIcon from '@lucide/svelte/icons/clock';
    import { TimeField } from 'bits-ui';

    import { i18n } from '$lib/i18n';

    import Field from './Field.svelte';

    interface Props {
        label: string;
        hideLabel?: boolean;
        hint?: string;
        error?: string;
        required?: boolean;
        /** Hora ISO `HH:mm` (o `HH:mm:ss` con `granularity="second"`). */
        value?: string;
        min?: string;
        max?: string;
        granularity?: 'minute' | 'second';
        /** Por defecto, el del idioma. */
        hourCycle?: 12 | 24;
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
        granularity = 'minute',
        hourCycle,
        disabled = false,
        readonly = false,
        onValueChange,
    }: Props = $props();

    const uid = $props.id();
    const labelId = `time-${uid}-label`;

    function toTime(iso: string | undefined): Time | undefined {
        const match = /^(\d{2}):(\d{2})(?::(\d{2}))?$/.exec(iso ?? '');
        if (!match) return undefined;
        const [hour, minute, second] = [Number(match[1]), Number(match[2]), Number(match[3] ?? 0)];
        if (hour > 23 || minute > 59 || second > 59) return undefined;
        return new Time(hour, minute, second);
    }

    const pad = (part: number) => String(part).padStart(2, '0');

    function update(next: Time | undefined): void {
        value = next
            ? `${pad(next.hour)}:${pad(next.minute)}${granularity === 'second' ? `:${pad(next.second)}` : ''}`
            : undefined;
        onValueChange?.(value);
    }
</script>

<Field {label} {hideLabel} {hint} {error} {required} {labelId}>
    {#snippet children({ describedBy, invalid })}
        <TimeField.Root
            bind:value={() => toTime(value), update}
            locale={i18n.current.intl}
            {granularity}
            {hourCycle}
            minValue={toTime(min)}
            maxValue={toTime(max)}
            {disabled}
            {readonly}
            {required}
        >
            <TimeField.Input
                class="bs-control"
                aria-labelledby={labelId}
                aria-describedby={describedBy}
                data-invalid={invalid || undefined}
            >
                {#snippet children({ segments })}
                    <span class="bs-control-icon"><ClockIcon /></span>
                    <div class="bs-segments">
                        {#each segments as { part, value: text }, index (index)}
                            <TimeField.Segment {part} class="bs-segment">{text}</TimeField.Segment>
                        {/each}
                    </div>
                {/snippet}
            </TimeField.Input>
        </TimeField.Root>
    {/snippet}
</Field>
