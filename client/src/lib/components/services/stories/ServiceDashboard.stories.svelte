<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'ServiceDashboard',
        group: 'Servicios',
        description: 'Resumen del servicio: CPU, memoria, swap, procesos, tiempo en marcha y reinicios.',
    };
</script>

<script lang="ts">
    import type { ServiceStatus } from '$lib/agent';
    import { Story } from '$lib/workbench';

    import ServiceDashboard from '../ServiceDashboard.svelte';

    const MIB = 1024 ** 2;
    const now = 1_800_000_000_000;
    const cpu = [8, 9, 12, 15, 30, 44, 38, 22, 18, 16, 14, 13, 20, 26, 24, 19, 15, 12, 11, 13];
    const history = cpu.map((value, index) => ({
        at: now - (cpu.length - index) * 3000,
        cpu: value,
        memory: (300 + index * 9 + (index % 4) * 6) * MIB,
    }));
    const running: ServiceStatus = {
        name: 'api',
        unit: 'blacksmith@api.service',
        loadState: 'loaded',
        activeState: 'active',
        subState: 'running',
        pid: 1234,
        metrics: null,
        deployed: true,
        release: 'r1799990000-000000000',
        previousRelease: 'r1799900000-000000000',
        resources: {
            memoryBytes: 471 * MIB,
            memoryPeakBytes: 520 * MIB,
            memoryLimitBytes: 512 * MIB,
            swapBytes: 12 * MIB,
            cpuPercent: 13,
            tasks: 11,
            restarts: 2,
            uptimeSecs: 3 * 86400 + 4 * 3600,
        },
        host: { cpuCores: 4, memoryBytes: 8192 * MIB, swapBytes: 2048 * MIB },
    };
    const stopped: ServiceStatus = {
        ...running,
        activeState: 'inactive',
        subState: 'dead',
        pid: null,
        resources: {
            ...running.resources,
            memoryBytes: null,
            swapBytes: null,
            cpuPercent: null,
            tasks: null,
            uptimeSecs: null,
        },
    };
    const site = {
        name: 'api',
        template: 'api' as const,
        domains: ['api.example.com'],
        port: 3000,
        https: true,
        email: null,
        routes: [],
        tls: 'active' as const,
        tlsDetail: null,
        configPath: '/var/lib/blacksmith/nginx/sites/api.conf',
        updatedAt: 1,
    };
</script>

<Story name="En marcha" layout="stack">
    <div class="frame"><ServiceDashboard status={running} {history} branch="main" {site} {now} /></div>
</Story>

<Story name="Parado" layout="stack">
    <div class="frame"><ServiceDashboard status={stopped} history={[]} branch="main" site={null} {now} /></div>
</Story>

<style>
    .frame {
        width: min(100%, var(--layout-content-max-width));
    }
</style>
