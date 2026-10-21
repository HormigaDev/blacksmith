<!--
    Único input de texto de la aplicación. `type="number"` sustituye las flechas nativas por dos
    controles flotantes, por fuera del borde, que aparecen al enfocar (mantener pulsado repite).
-->
<script lang="ts" generics="V extends string | number | null | undefined">
    import EyeIcon from '@lucide/svelte/icons/eye';
    import EyeOffIcon from '@lucide/svelte/icons/eye-off';
    import MinusIcon from '@lucide/svelte/icons/minus';
    import PlusIcon from '@lucide/svelte/icons/plus';
    import type { Snippet } from 'svelte';
    import type { HTMLInputAttributes } from 'svelte/elements';

    import { t } from '$lib/i18n';

    import Field from './Field.svelte';

    type InputType = 'text' | 'email' | 'password' | 'search' | 'url' | 'tel' | 'number';

    interface Props extends Omit<HTMLInputAttributes, 'value' | 'type' | 'size' | 'children'> {
        label: string;
        hideLabel?: boolean;
        hint?: string;
        error?: string;
        type?: InputType;
        value?: V;
        size?: 'sm' | 'md' | 'lg';
        /** Ícono o contenido a la izquierda. */
        leading?: Snippet;
        /** Íconos o acciones a la derecha. */
        trailing?: Snippet;
    }

    let {
        label,
        hideLabel = false,
        hint,
        error,
        required = false,
        type = 'text',
        value = $bindable(),
        size = 'md',
        leading,
        trailing,
        disabled,
        readonly,
        id,
        ...rest
    }: Props = $props();

    let input = $state<HTMLInputElement>();
    let revealed = $state(false);
    const inputType = $derived(type === 'password' && revealed ? 'text' : type);
    const interactive = $derived(!disabled && !readonly);

    const numeric = $derived(typeof value === 'number' ? value : Number.NaN);
    const canDecrement = $derived(rest.min === undefined || Number.isNaN(numeric) || numeric > Number(rest.min));
    const canIncrement = $derived(rest.max === undefined || Number.isNaN(numeric) || numeric < Number(rest.max));

    function step(direction: 1 | -1): void {
        if (!input || !interactive) return;
        try {
            if (direction > 0) input.stepUp();
            else input.stepDown();
        } catch {
            return;
        }
        // El cambio programático no dispara eventos: se emiten para que `bind:value` y los
        // manejadores del padre se enteren, como si el usuario hubiera escrito.
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
    }

    const REPEAT_DELAY_MS = 400;
    const REPEAT_INTERVAL_MS = 60;
    let repeat: ReturnType<typeof setTimeout> | undefined;

    function startStepping(event: PointerEvent, direction: 1 | -1): void {
        // Mantiene el foco en el input: los controles solo son visibles mientras lo tenga.
        event.preventDefault();
        if (event.button !== 0) return;
        step(direction);
        const tick = () => {
            step(direction);
            repeat = setTimeout(tick, REPEAT_INTERVAL_MS);
        };
        repeat = setTimeout(tick, REPEAT_DELAY_MS);
    }

    function stopStepping(): void {
        clearTimeout(repeat);
        repeat = undefined;
    }

    $effect(() => stopStepping);
</script>

<Field {label} {hideLabel} {hint} {error} required={required ?? false} id={id ?? undefined}>
    {#snippet children({ id: controlId, describedBy, invalid })}
        <div class="wrapper">
            <div
                class="bs-control"
                class:bs-control-sm={size === 'sm'}
                class:bs-control-lg={size === 'lg'}
                data-invalid={invalid || undefined}
                data-disabled={disabled || undefined}
            >
                {#if leading}
                    <span class="bs-control-icon">{@render leading()}</span>
                {/if}

                <input
                    {...rest}
                    bind:this={input}
                    bind:value
                    id={controlId}
                    type={inputType}
                    class="bs-control-input"
                    {required}
                    {disabled}
                    {readonly}
                    aria-invalid={invalid || undefined}
                    aria-describedby={describedBy}
                />

                {#if type === 'password'}
                    <button
                        type="button"
                        class="bs-icon-button"
                        aria-label={revealed ? t('components.input.hidePassword') : t('components.input.showPassword')}
                        aria-pressed={revealed}
                        {disabled}
                        onclick={() => (revealed = !revealed)}
                    >
                        {#if revealed}<EyeOffIcon />{:else}<EyeIcon />{/if}
                    </button>
                {/if}

                {#if trailing}
                    <span class="bs-control-icon">{@render trailing()}</span>
                {/if}
            </div>

            {#if type === 'number' && interactive}
                <div class="steppers">
                    <button
                        type="button"
                        class="bs-icon-button stepper"
                        tabindex="-1"
                        aria-label={t('components.input.decrement')}
                        disabled={!canDecrement}
                        onpointerdown={(event) => startStepping(event, -1)}
                        onpointerup={stopStepping}
                        onpointerleave={stopStepping}
                        onpointercancel={stopStepping}
                    >
                        <MinusIcon />
                    </button>
                    <button
                        type="button"
                        class="bs-icon-button stepper"
                        tabindex="-1"
                        aria-label={t('components.input.increment')}
                        disabled={!canIncrement}
                        onpointerdown={(event) => startStepping(event, 1)}
                        onpointerup={stopStepping}
                        onpointerleave={stopStepping}
                        onpointercancel={stopStepping}
                    >
                        <PlusIcon />
                    </button>
                </div>
            {/if}
        </div>
    {/snippet}
</Field>

<style>
    .wrapper {
        position: relative;
    }

    /* Flotan por encima del borde superior derecho; solo con el input enfocado. */
    .steppers {
        position: absolute;
        right: var(--space-0);
        bottom: calc(100% + var(--space-1));
        z-index: var(--z-raised);
        display: flex;
        gap: var(--space-1);
        padding: var(--space-1);
        background-color: var(--color-surface-raised);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
        box-shadow: var(--shadow-md);
        opacity: 0;
        visibility: hidden;
        transform: translateY(var(--space-1));
        transition:
            opacity var(--duration-fast) var(--easing-standard),
            transform var(--duration-fast) var(--easing-standard),
            visibility var(--duration-fast) var(--easing-standard);
    }

    .wrapper:focus-within .steppers {
        opacity: 1;
        visibility: visible;
        transform: none;
    }

    .stepper {
        width: var(--control-height-sm);
        height: var(--control-height-sm);
    }

    .stepper:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
</style>
