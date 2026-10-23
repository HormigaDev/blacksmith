<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'HostOverview',
        group: 'Servidores',
        description: 'Datos del servidor: sistema, CPU y carga, memoria, swap, tarjeta gráfica, discos y red.',
    };
</script>

<script lang="ts">
    import type { HostInfo } from '$lib/agent';
    import { Story } from '$lib/workbench';

    import HostOverview from '../HostOverview.svelte';

    const GIB = 1024 ** 3;
    const vps: HostInfo = {
        hostname: 'vps-produccion-1',
        os: { name: 'Ubuntu 24.04.1 LTS', id: 'ubuntu', kernel: '6.8.0-45-generic', architecture: 'x86_64' },
        machine: 'Hetzner vServer',
        virtualization: 'kvm',
        cpu: {
            model: 'AMD EPYC-Rome Processor',
            vendor: 'AuthenticAMD',
            physicalCores: 4,
            logicalCores: 4,
            frequencyMhz: 2445,
        },
        gpus: [],
        memory: { totalBytes: 8 * GIB, availableBytes: 5.2 * GIB, swapTotalBytes: 2 * GIB, swapFreeBytes: 1.8 * GIB },
        disks: [
            {
                mountPoint: '/',
                device: '/dev/sda1',
                fileSystem: 'ext4',
                kind: 'ssd',
                totalBytes: 160 * GIB,
                availableBytes: 112 * GIB,
                readOnly: false,
            },
            {
                mountPoint: '/mnt/volumen',
                device: '/dev/sdb',
                fileSystem: 'xfs',
                kind: 'hdd',
                totalBytes: 500 * GIB,
                availableBytes: 30 * GIB,
                readOnly: false,
            },
        ],
        network: [
            {
                name: 'eth0',
                addresses: ['203.0.113.7/32', '2001:db8:1::1/64'],
                receivedBytes: 182 * GIB,
                transmittedBytes: 64 * GIB,
            },
            { name: 'wg0', addresses: ['10.8.0.1/24'], receivedBytes: 1.2 * GIB, transmittedBytes: 0.9 * GIB },
        ],
        uptimeSecs: 41 * 86400 + 5 * 3600,
        bootTime: 1_796_000_000,
        loadAverage: [0.42, 0.37, 0.31],
        timezone: 'Europe/Madrid',
        agentVersion: '1.0.0',
    };
    const workstation: HostInfo = {
        ...vps,
        hostname: 'estudio',
        machine: 'ASUSTeK COMPUTER INC. ProArt',
        virtualization: null,
        cpu: {
            model: '12th Gen Intel(R) Core(TM) i7-1255U',
            vendor: 'GenuineIntel',
            physicalCores: 10,
            logicalCores: 12,
            frequencyMhz: 2952,
        },
        gpus: [
            {
                name: 'Navi 21 [Radeon RX 6800/6800 XT / 6900 XT]',
                vendor: 'Advanced Micro Devices, Inc. [AMD/ATI]',
                driver: 'amdgpu',
                memoryBytes: 16 * GIB,
                address: '0000:03:00.0',
            },
        ],
        memory: { totalBytes: 16 * GIB, availableBytes: 1.1 * GIB, swapTotalBytes: 0, swapFreeBytes: 0 },
        loadAverage: [11.2, 9.8, 7.5],
    };
</script>

<Story name="VPS" layout="stack">
    <div class="frame"><HostOverview host={vps} /></div>
</Story>

<Story name="Equipo con GPU y sin swap" layout="stack">
    <div class="frame"><HostOverview host={workstation} /></div>
</Story>

<style>
    .frame {
        width: min(100%, var(--layout-content-max-width));
    }
</style>
