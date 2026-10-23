<!--
    Límites de recursos del servicio: CPU (y prioridad frente a los demás), memoria, swap y
    procesos. Un campo vacío es «sin límite». Valida como el agente (`limits.rs`) y los límites se
    aplican al momento, sin reiniciar el servicio (systemd los pone en su cgroup).

    El consumo actual (`usage`) se muestra junto a cada límite para elegirlo con criterio.
-->
<script lang="ts">
    import CpuIcon from '@lucide/svelte/icons/cpu';
    import ListTreeIcon from '@lucide/svelte/icons/list-tree';
    import MemoryStickIcon from '@lucide/svelte/icons/memory-stick';
    import type { Component } from 'svelte';
    import { untrack } from 'svelte';

    import {
        MAX_TASKS,
        MIN_MEMORY_BYTES,
        MIN_TASKS,
        type Priority,
        type ResourceLimits,
        type ServiceLimits,
        type ServiceResources,
    } from '$lib/agent';
    import { describeError } from '$lib/http';
    import { formatBytes, formatNumber, formatPercent, t } from '$lib/i18n';

    import Button from '../ui/Button.svelte';
    import Input from '../ui/Input.svelte';
    import Select from '../ui/Select.svelte';
    import ToggleGroup from '../ui/ToggleGroup.svelte';

    interface Props {
        limits: ServiceLimits;
        /** Consumo actual del servicio (`null` si no se conoce). */
        usage?: ServiceResources | null;
        onSave: (limits: ResourceLimits) => Promise<void>;
    }

    let { limits, usage = null, onSave }: Props = $props();

    const MIB = 1024 * 1024;
    const GIB = 1024 * MIB;
    type Unit = 'MB' | 'GB';
    type SwapMode = 'unlimited' | 'none' | 'limit';

    const host = $derived(limits.host);
    const maxCpu = $derived(Math.max(100, host.cpuCores * 100));
    const sectionId = $props.id();

    /** Bytes a número y unidad: en GB si es un número entero de GB. */
    function split(bytes: number | null): [number | string, Unit] {
        if (bytes === null) return ['', 'MB'];
        return bytes % GIB === 0 ? [bytes / GIB, 'GB'] : [Math.round(bytes / MIB), 'MB'];
    }

    const initial = untrack(() => limits.limits);
    // Los campos numéricos entregan un número (o cadena vacía si se borran).
    let cpu = $state<number | string>(initial.cpuPercent ?? '');
    let priority = $state<Priority>(initial.priority);
    const [initialMemory, initialMemoryUnit] = split(initial.memoryMaxBytes);
    let memory = $state(initialMemory);
    let memoryUnit = $state<Unit>(initialMemoryUnit);
    let swapMode = $state<SwapMode>(
        initial.swapMaxBytes === null ? 'unlimited' : initial.swapMaxBytes === 0 ? 'none' : 'limit',
    );
    const [initialSwap, initialSwapUnit] = split(initial.swapMaxBytes || null);
    let swap = $state(initialSwap);
    let swapUnit = $state<Unit>(initialSwapUnit);
    let tasks = $state<number | string>(initial.tasksMax ?? '');
    let submitted = $state(false);
    let busy = $state(false);
    let formError = $state<string>();

    const text = (value: number | string) => String(value).trim();
    const integer = (value: number | string) => {
        const number = Number(text(value));
        return text(value) && Number.isInteger(number) ? number : null;
    };
    const bytes = (value: number | string, unit: Unit) => {
        const number = Number(text(value));
        return text(value) && Number.isFinite(number) && number > 0
            ? Math.round(number * (unit === 'GB' ? GIB : MIB))
            : null;
    };

    const cpuError = $derived.by(() => {
        if (!text(cpu)) return undefined;
        const value = integer(cpu);
        return value === null || value < 1 || value > maxCpu
            ? t('services.limits.cpuInvalid', { max: formatNumber(maxCpu), cores: formatNumber(host.cpuCores) })
            : undefined;
    });
    const memoryBytes = $derived(bytes(memory, memoryUnit));
    const memoryError = $derived.by(() => {
        if (!text(memory)) return undefined;
        return memoryBytes === null ||
            memoryBytes < MIN_MEMORY_BYTES ||
            (host.memoryBytes > 0 && memoryBytes > host.memoryBytes)
            ? t('services.limits.memoryInvalid', {
                  min: formatBytes(MIN_MEMORY_BYTES),
                  max: formatBytes(host.memoryBytes),
              })
            : undefined;
    });
    const swapBytes = $derived(bytes(swap, swapUnit));
    const swapError = $derived(
        swapMode === 'limit' && (swapBytes === null || swapBytes > host.swapBytes)
            ? t('services.limits.swapInvalid', { max: formatBytes(host.swapBytes) })
            : undefined,
    );
    const tasksError = $derived.by(() => {
        if (!text(tasks)) return undefined;
        const value = integer(tasks);
        return value === null || value < MIN_TASKS || value > MAX_TASKS
            ? t('services.limits.tasksInvalid', { min: formatNumber(MIN_TASKS), max: formatNumber(MAX_TASKS) })
            : undefined;
    });

    const unitOptions = [
        { value: 'MB' as const, label: 'MB' },
        { value: 'GB' as const, label: 'GB' },
    ];
    const priorityOptions = $derived(
        (['low', 'normal', 'high'] as const).map((value) => ({
            value,
            label: t(`services.limits.priorities.${value}`),
        })),
    );
    const swapOptions = $derived([
        { value: 'unlimited' as const, label: t('services.limits.swapUnlimited') },
        { value: 'none' as const, label: t('services.limits.swapNone') },
        { value: 'limit' as const, label: t('services.limits.swapLimit'), disabled: host.swapBytes === 0 },
    ]);

    const usageCpu = $derived(usage?.cpuPercent != null ? formatPercent(usage.cpuPercent / 100) : null);
    const usageMemory = $derived(usage?.memoryBytes != null ? formatBytes(usage.memoryBytes) : null);
    const usagePeak = $derived(usage?.memoryPeakBytes != null ? formatBytes(usage.memoryPeakBytes) : null);
    const usageSwap = $derived(usage?.swapBytes != null ? formatBytes(usage.swapBytes) : null);
    const usageTasks = $derived(usage?.tasks != null ? formatNumber(usage.tasks) : null);

    const anyLimit = $derived(
        Boolean(text(cpu) || text(memory) || text(tasks)) || swapMode !== 'unlimited' || priority !== 'normal',
    );

    function clear(): void {
        cpu = '';
        priority = 'normal';
        memory = '';
        swapMode = 'unlimited';
        swap = '';
        tasks = '';
        submitted = false;
    }

    async function save(event: SubmitEvent): Promise<void> {
        event.preventDefault();
        submitted = true;
        formError = undefined;
        if (cpuError || memoryError || swapError || tasksError) return;
        busy = true;
        try {
            await onSave({
                cpuPercent: integer(cpu),
                priority,
                memoryMaxBytes: memoryBytes,
                swapMaxBytes: swapMode === 'unlimited' ? null : swapMode === 'none' ? 0 : swapBytes,
                tasksMax: integer(tasks),
            });
            submitted = false;
        } catch (error) {
            formError = describeError(error);
        } finally {
            busy = false;
        }
    }
</script>

{#snippet header(key: string, Icon: Component, title: string, hint: string)}
    <header class="section-header">
        <span class="section-icon" aria-hidden="true"><Icon /></span>
        <div>
            <h3 class="section-title" id="{sectionId}-{key}">{title}</h3>
            <p class="section-hint">{hint}</p>
        </div>
    </header>
{/snippet}

<form class="form" novalidate onsubmit={save}>
    <p class="intro">{t('services.limits.intro')}</p>

    <section class="section" aria-labelledby="{sectionId}-cpu">
        {@render header(
            'cpu',
            CpuIcon,
            t('services.limits.cpu'),
            t('services.limits.cpuHint', { cores: formatNumber(host.cpuCores), max: formatNumber(maxCpu) }),
        )}
        <div class="row">
            <div class="grow">
                <Input
                    label={t('services.limits.cpuMax')}
                    hint={usageCpu ? t('services.limits.usingNow', { value: usageCpu }) : undefined}
                    type="number"
                    min={1}
                    max={maxCpu}
                    placeholder={t('services.limits.unlimited')}
                    error={submitted ? cpuError : undefined}
                    bind:value={cpu}
                >
                    {#snippet trailing()}<span class="suffix">%</span>{/snippet}
                </Input>
            </div>
            <div class="grow field">
                <span class="label" aria-hidden="true">{t('services.limits.priority')}</span>
                <ToggleGroup label={t('services.limits.priority')} options={priorityOptions} bind:value={priority} />
                <p class="hint">{t(`services.limits.priorityHints.${priority}`)}</p>
            </div>
        </div>
    </section>

    <section class="section" aria-labelledby="{sectionId}-memory">
        {@render header(
            'memory',
            MemoryStickIcon,
            t('services.limits.memory'),
            t('services.limits.memoryHint', { total: formatBytes(host.memoryBytes) }),
        )}
        <div class="row">
            <div class="grow amount">
                <div class="amount-value">
                    <Input
                        label={t('services.limits.memoryMax')}
                        hint={usageMemory
                            ? t('services.limits.memoryUsing', { value: usageMemory, peak: usagePeak ?? usageMemory })
                            : undefined}
                        type="number"
                        min={1}
                        placeholder={t('services.limits.unlimited')}
                        error={submitted ? memoryError : undefined}
                        bind:value={memory}
                    />
                </div>
                <div class="amount-unit">
                    <Select label={t('services.limits.unit')} options={unitOptions} bind:value={memoryUnit} />
                </div>
            </div>
            <div class="grow field">
                <span class="label" aria-hidden="true">{t('services.limits.swap')}</span>
                <ToggleGroup label={t('services.limits.swap')} options={swapOptions} bind:value={swapMode} />
                <p class="hint">
                    {host.swapBytes === 0
                        ? t('services.limits.swapHostNone')
                        : usageSwap
                          ? t('services.limits.swapUsing', { value: usageSwap, total: formatBytes(host.swapBytes) })
                          : t('services.limits.swapHint', { total: formatBytes(host.swapBytes) })}
                </p>
                {#if swapMode === 'limit'}
                    <div class="amount">
                        <div class="amount-value">
                            <Input
                                label={t('services.limits.swapMax')}
                                type="number"
                                min={1}
                                error={submitted ? swapError : undefined}
                                bind:value={swap}
                            />
                        </div>
                        <div class="amount-unit">
                            <Select label={t('services.limits.unit')} options={unitOptions} bind:value={swapUnit} />
                        </div>
                    </div>
                {/if}
            </div>
        </div>
    </section>

    <section class="section" aria-labelledby="{sectionId}-tasks">
        {@render header('tasks', ListTreeIcon, t('services.limits.tasks'), t('services.limits.tasksHint'))}
        <div class="row">
            <div class="grow">
                <Input
                    label={t('services.limits.tasksMax')}
                    hint={usageTasks ? t('services.limits.usingNow', { value: usageTasks }) : undefined}
                    type="number"
                    min={MIN_TASKS}
                    max={MAX_TASKS}
                    placeholder={t('services.limits.unlimited')}
                    error={submitted ? tasksError : undefined}
                    bind:value={tasks}
                />
            </div>
            <div class="grow"></div>
        </div>
    </section>

    {#if formError}<p class="alert" role="alert">{formError}</p>{/if}

    <div class="actions">
        {#if anyLimit}
            <Button variant="ghost" onclick={clear}>{t('services.limits.clear')}</Button>
        {/if}
        <Button type="submit" variant="primary" loading={busy}>{t('services.limits.save')}</Button>
    </div>
</form>

<style>
    .form {
        display: flex;
        flex-direction: column;
        gap: var(--space-6);
    }

    .intro {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .section {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
    }

    .section + .section {
        padding-top: var(--space-6);
        border-top: var(--border-width) solid var(--color-border);
    }

    .section-header {
        display: flex;
        gap: var(--space-3);
        align-items: flex-start;
    }

    /* Icono de la sección: un recuadro del color de acento, el ancla visual de cada bloque. */
    .section-icon {
        display: inline-flex;
        flex-shrink: 0;
        align-items: center;
        justify-content: center;
        width: var(--control-height-md);
        height: var(--control-height-md);
        color: var(--color-accent);
        background-color: var(--color-accent-subtle);
        border-radius: var(--radius-md);
    }

    .section-icon :global(svg) {
        width: var(--icon-size-md);
        height: var(--icon-size-md);
    }

    .section-title {
        font-size: var(--font-size-md);
        font-weight: var(--font-weight-semibold);
        color: var(--color-text);
    }

    .section-hint {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .row {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-4);
    }

    .grow {
        flex: 1 1 min(100%, var(--layout-card-min-width));
        min-width: 0;
    }

    .field {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        align-items: flex-start;
    }

    .label {
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-medium);
        color: var(--color-text);
    }

    .hint {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    /* Cantidad con su unidad: el número crece y la unidad queda pegada a su derecha. */
    .amount {
        display: flex;
        gap: var(--space-2);
        align-items: flex-start;
        align-self: stretch;
    }

    .amount-value {
        flex: 1 1 auto;
        min-width: 0;
    }

    .amount-unit {
        flex: 0 0 var(--unit-select-width);
    }

    .suffix {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }

    .actions {
        display: flex;
        flex-wrap: wrap;
        justify-content: flex-end;
        gap: var(--space-2);
    }
</style>
