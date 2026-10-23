import { describe, expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { HostInfo } from '$lib/agent';

import HostOverview from '../HostOverview.svelte';

const GIB = 1024 ** 3;
const HOST: HostInfo = {
    hostname: 'vps-1',
    os: { name: 'Debian GNU/Linux 12 (bookworm)', id: 'debian', kernel: '6.1.0-25-amd64', architecture: 'x86_64' },
    machine: 'QEMU Standard PC (Q35 + ICH9, 2009)',
    virtualization: 'kvm',
    cpu: { model: 'AMD EPYC 7763', vendor: 'AuthenticAMD', physicalCores: 2, logicalCores: 4, frequencyMhz: 2450 },
    gpus: [],
    memory: { totalBytes: 4 * GIB, availableBytes: 3 * GIB, swapTotalBytes: 0, swapFreeBytes: 0 },
    disks: [
        {
            mountPoint: '/',
            device: '/dev/vda1',
            fileSystem: 'ext4',
            kind: 'ssd',
            totalBytes: 80 * GIB,
            availableBytes: 60 * GIB,
            readOnly: false,
        },
    ],
    network: [{ name: 'eth0', addresses: ['203.0.113.7/24'], receivedBytes: 2 * GIB, transmittedBytes: GIB }],
    uptimeSecs: 3 * 86400,
    bootTime: 1_790_000_000,
    loadAverage: [0.5, 0.25, 0.1],
    timezone: 'Europe/Madrid',
    agentVersion: '1.2.3',
};

describe('HostOverview', () => {
    test('sistema, equipo y virtualización', async () => {
        await render(HostOverview, { host: HOST });
        await expect.element(page.getByText('Debian GNU/Linux 12 (bookworm)')).toBeVisible();
        await expect.element(page.getByText('6.1.0-25-amd64 (x86_64)')).toBeVisible();
        await expect.element(page.getByText('Virtual · KVM')).toBeVisible();
        await expect.element(page.getByText('v1.2.3')).toBeVisible();
        await expect.element(page.getByText('Europe/Madrid')).toBeVisible();
    });

    test('CPU con núcleos e hilos, carga, memoria y sin swap ni GPU', async () => {
        await render(HostOverview, { host: HOST });
        await expect.element(page.getByText('AMD EPYC 7763')).toBeVisible();
        await expect.element(page.getByText('4 hilos', { exact: true })).toBeVisible();
        await expect.element(page.getByText('2 núcleos · 2,5 GHz')).toBeVisible();
        await expect
            .element(page.getByRole('meter', { name: /Carga del último minuto frente a 4 hilos/ }))
            .toBeVisible();
        await expect.element(page.getByText('3 GB disponibles de 4 GB')).toBeVisible();
        await expect.element(page.getByText('Sin swap')).toBeVisible();
        await expect.element(page.getByText('Ninguna')).toBeVisible();
    });

    test('discos con su espacio y red con direcciones y tráfico', async () => {
        await render(HostOverview, { host: HOST });
        await expect.element(page.getByRole('meter', { name: 'Espacio usado en /' })).toBeVisible();
        await expect.element(page.getByText('60 GB libres de 80 GB')).toBeVisible();
        await expect.element(page.getByText('203.0.113.7/24')).toBeVisible();
        await expect.element(page.getByText('eth0')).toBeVisible();
    });

    test('lo desconocido no se inventa', async () => {
        await render(HostOverview, {
            host: {
                ...HOST,
                hostname: null,
                machine: null,
                virtualization: null,
                timezone: null,
                disks: [],
                network: [],
                cpu: { ...HOST.cpu, model: null, physicalCores: null, frequencyMhz: null },
            },
        });
        await expect.element(page.getByText('4 hilos', { exact: true })).toBeVisible();
        await expect.element(page.getByText(/núcleos|GHz/)).not.toBeInTheDocument();
        await expect.element(page.getByText(/Virtual/)).not.toBeInTheDocument();
        await expect.element(page.getByRole('heading', { name: 'Discos' })).not.toBeInTheDocument();
        await expect.element(page.getByRole('heading', { name: 'Red' })).not.toBeInTheDocument();
    });
});
