<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'ResourceLimitsForm',
        group: 'Servicios',
        description: 'Límites de CPU, prioridad, memoria, swap y procesos del servicio, con su consumo actual.',
    };
</script>

<script lang="ts">
    import type { ServiceLimits, ServiceResources } from '$lib/agent';
    import { Story } from '$lib/workbench';

    import ResourceLimitsForm from '../ResourceLimitsForm.svelte';

    const MIB = 1024 ** 2;
    const GIB = 1024 * MIB;
    const host = { cpuCores: 4, memoryBytes: 8 * GIB, swapBytes: 2 * GIB };
    const unlimited: ServiceLimits = {
        limits: { cpuPercent: null, priority: 'normal', memoryMaxBytes: null, swapMaxBytes: null, tasksMax: null },
        host,
        updatedAt: null,
    };
    const limited: ServiceLimits = {
        limits: { cpuPercent: 150, priority: 'high', memoryMaxBytes: GIB, swapMaxBytes: 0, tasksMax: 512 },
        host,
        updatedAt: 1,
    };
    const usage: ServiceResources = {
        memoryBytes: 412 * MIB,
        memoryPeakBytes: 530 * MIB,
        memoryLimitBytes: GIB,
        swapBytes: 0,
        swapLimitBytes: 0,
        cpuPercent: 23.5,
        cpuLimitPercent: 150,
        tasks: 14,
        tasksLimit: 512,
        restarts: 0,
        uptimeSecs: 3600,
    };
    const onSave = async () => {};
</script>

<Story name="Sin límites" layout="stack">
    <div class="frame"><ResourceLimitsForm limits={unlimited} {onSave} /></div>
</Story>

<Story name="Con límites" layout="stack">
    <div class="frame"><ResourceLimitsForm limits={limited} {usage} {onSave} /></div>
</Story>

<Story name="Servidor sin swap" layout="stack">
    <div class="frame">
        <ResourceLimitsForm limits={{ ...unlimited, host: { ...host, swapBytes: 0 } }} {usage} {onSave} />
    </div>
</Story>

<style>
    .frame {
        width: min(100%, var(--dialog-width-lg));
    }
</style>
