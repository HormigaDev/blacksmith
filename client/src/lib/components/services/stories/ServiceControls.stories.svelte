<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'ServiceControls',
        group: 'Servicios',
        description: 'Estado del servicio, consumo y acciones de ciclo de vida.',
    };
</script>

<script lang="ts">
    import type { ServiceStatus } from '$lib/agent';
    import { Story } from '$lib/workbench';

    import ServiceControls from '../ServiceControls.svelte';

    const base: ServiceStatus = {
        name: 'api',
        unit: 'blacksmith-api.service',
        loadState: 'loaded',
        activeState: 'inactive',
        subState: 'dead',
        pid: null,
        deployed: true,
        release: 'r1800000000-000000000',
        previousRelease: null,
        resources: {
            memoryBytes: null,
            memoryPeakBytes: null,
            memoryLimitBytes: null,
            swapLimitBytes: null,
            cpuLimitPercent: null,
            tasksLimit: null,
            swapBytes: null,
            cpuPercent: null,
            tasks: null,
            restarts: null,
            uptimeSecs: null,
        },
        host: { cpuCores: 4, memoryBytes: 8 * 1024 ** 3, swapBytes: 0 },
    };
    const noop = () => {};
</script>

<Story name="Sin desplegar" layout="stack">
    <ServiceControls name="API" status={{ ...base, deployed: false }} onAction={noop} />
</Story>

<Story name="Detenido" layout="stack">
    <ServiceControls name="API" status={base} onAction={noop} />
</Story>

<Story name="En marcha" layout="stack">
    <ServiceControls
        name="API"
        status={{
            ...base,
            activeState: 'active',
            subState: 'running',
            pid: 812,
            resources: { ...base.resources, memoryBytes: 188_743_680, cpuPercent: 3.2, tasks: 9, uptimeSecs: 3600 },
        }}
        onAction={noop}
    />
</Story>

<Story name="Reiniciando" layout="stack">
    <ServiceControls name="API" status={{ ...base, activeState: 'active' }} busy="restart" onAction={noop} />
</Story>

<Story name="Fallido" layout="stack">
    <ServiceControls name="API" status={{ ...base, activeState: 'failed', subState: 'failed' }} onAction={noop} />
</Story>

<Story name="Sitio estático">
    <ServiceControls name="Web" status={base} staticOnly onAction={noop} />
</Story>
