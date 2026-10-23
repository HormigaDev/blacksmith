import { describe, expect, test } from 'vitest';

import {
    parseHostInfo,
    parseLogEvent,
    parseServiceLimits,
    parseServiceDeployment,
    parseSite,
    parseServiceEnvironment,
    parseServiceList,
    parseServiceStatus,
    SERVICE_NAME_PATTERN,
} from './types';

describe('parseServiceList', () => {
    test('acepta el JSON del agente y rechaza formas inesperadas', () => {
        const service = {
            name: 'api',
            displayName: 'API',
            description: '',
            unit: 'blacksmith-api.service',
            createdAt: 1,
        };
        expect(parseServiceList([service])).toEqual([service]);
        expect(() => parseServiceList({})).toThrow(TypeError);
        expect(() => parseServiceList([{ ...service, createdAt: '1' }])).toThrow(TypeError);
    });
});

describe('parseLogEvent', () => {
    test('entradas y fin', () => {
        expect(
            parseLogEvent({ type: 'entry', timestampUs: 1, priority: 6, pid: null, identifier: 'x', message: 'hola' }),
        ).toEqual({ type: 'entry', timestampUs: 1, priority: 6, pid: null, identifier: 'x', message: 'hola' });
        expect(parseLogEvent({ type: 'end', reason: 'agentShutdown', detail: null })).toEqual({
            type: 'end',
            reason: 'agentShutdown',
            detail: null,
        });
    });

    test('rechaza lo desconocido', () => {
        for (const event of [
            null,
            { type: 'otro' },
            { type: 'entry', timestampUs: '1', message: 'x' },
            { type: 'entry', timestampUs: 1, message: 'x', priority: 'alta' },
            { type: 'end', reason: 'porque-si', detail: null },
        ]) {
            expect(() => parseLogEvent(event), JSON.stringify(event)).toThrow(TypeError);
        }
    });
});

test('SERVICE_NAME_PATTERN: mismas reglas que el agente', () => {
    for (const ok of ['api', 'a', 'mi-api-2', 'a'.repeat(48)]) expect(SERVICE_NAME_PATTERN.test(ok), ok).toBe(true);
    for (const bad of ['', 'API', '-a', 'a-', 'a_b', 'a.b', 'a'.repeat(49)]) {
        expect(SERVICE_NAME_PATTERN.test(bad), bad).toBe(false);
    }
});

describe('parseServiceStatus', () => {
    const status = {
        name: 'api',
        unit: 'blacksmith-api.service',
        loadState: 'loaded',
        activeState: 'active',
        subState: 'running',
        pid: 42,
        deployed: true,
        release: 'r1800000000-000000000',
        previousRelease: null,
        resources: {
            memoryBytes: 1024,
            memoryPeakBytes: 2048,
            memoryLimitBytes: null,
            swapLimitBytes: null,
            cpuLimitPercent: null,
            tasksLimit: null,
            swapBytes: 0,
            cpuPercent: 1.5,
            tasks: 3,
            restarts: 0,
            uptimeSecs: 60,
        },
        host: { cpuCores: 4, memoryBytes: 8192, swapBytes: 0 },
    };
    test('acepta el JSON del agente', () => {
        expect(parseServiceStatus(status)).toEqual(status);
        expect(parseServiceStatus({ ...status, pid: null }).pid).toBeNull();
    });
    test('rechaza estados desconocidos', () => {
        expect(() => parseServiceStatus({ ...status, activeState: 'bailando' })).toThrow(TypeError);
        expect(() => parseServiceStatus({ ...status, loadState: 'otro' })).toThrow(TypeError);
        expect(() => parseServiceStatus({ ...status, deployed: undefined })).toThrow(TypeError);
        expect(() => parseServiceStatus({ ...status, release: 1 })).toThrow(TypeError);
        expect(() => parseServiceStatus({ ...status, resources: { ...status.resources, tasks: '3' } })).toThrow(
            TypeError,
        );
        expect(() => parseServiceStatus({ ...status, host: null })).toThrow(TypeError);
    });
});

describe('parseServiceDeployment y parseServiceEnvironment', () => {
    test('despliegue con y sin repositorio', () => {
        const deployment = {
            name: 'api',
            repository: { owner: 'o', name: 'r', branch: 'main' },
            access: 'account',
            workingDirectory: '/srv/blacksmith/api',
            setupCommands: ['npm ci'],
            startCommand: 'npm start',
            publishDir: null,
            autoDeploy: true,
            updatedAt: 1,
        };
        expect(parseServiceDeployment(deployment)).toEqual(deployment);
        expect(parseServiceDeployment({ ...deployment, repository: null, updatedAt: null }).repository).toBeNull();
        expect(() => parseServiceDeployment({ ...deployment, setupCommands: [1] })).toThrow(TypeError);
        expect(() => parseServiceDeployment({ ...deployment, access: 'ssh' })).toThrow(TypeError);
    });

    test('sitio de nginx', () => {
        const site = {
            name: 'web',
            template: 'spa',
            domains: ['example.com'],
            port: null,
            https: true,
            email: null,
            routes: [{ path: '/api/', port: 3000, websocket: false }],
            tls: 'issuing',
            tlsDetail: null,
            configPath: '/var/lib/blacksmith/nginx/sites/web.conf',
            updatedAt: 1,
        };
        expect(parseSite(site)).toEqual(site);
        expect(parseSite(null)).toBeNull();
        expect(() => parseSite({ ...site, template: 'php' })).toThrow(TypeError);
        expect(() => parseSite({ ...site, tls: 'maybe' })).toThrow(TypeError);
        expect(() => parseSite({ ...site, routes: [{ path: '/api/', port: '3000' }] })).toThrow(TypeError);
        // Agente sin rutas.
        const { routes: _routes, ...legacy } = site;
        expect(parseSite(legacy)?.routes).toEqual([]);
    });

    test('variables del servicio', () => {
        const env = {
            name: 'api',
            unit: 'blacksmith-api.service',
            file: '/etc/blacksmith/env/blacksmith-api.env',
            loadedByUnit: true,
            globalLoadedByUnit: false,
            variables: [{ key: 'A', value: '1' }],
        };
        expect(parseServiceEnvironment(env)).toEqual(env);
        expect(() => parseServiceEnvironment({ ...env, variables: [{ key: 'A' }] })).toThrow(TypeError);
    });
});

describe('límites y servidor', () => {
    test('límites del servicio', () => {
        const limits = {
            limits: { cpuPercent: 150, priority: 'high', memoryMaxBytes: 1024, swapMaxBytes: 0, tasksMax: null },
            host: { cpuCores: 2, memoryBytes: 4096, swapBytes: 0 },
            updatedAt: 1,
        };
        expect(parseServiceLimits(limits)).toEqual(limits);
        expect(() => parseServiceLimits({ ...limits, limits: { ...limits.limits, priority: 'max' } })).toThrow(
            TypeError,
        );
        expect(() => parseServiceLimits({ ...limits, limits: { ...limits.limits, cpuPercent: '150' } })).toThrow(
            TypeError,
        );
    });

    test('datos del servidor', () => {
        const host = {
            hostname: null,
            os: { name: 'Ubuntu 24.04.1 LTS', id: 'ubuntu', kernel: null, architecture: 'x86_64' },
            machine: null,
            virtualization: 'kvm',
            cpu: { model: null, vendor: null, physicalCores: null, logicalCores: 2, frequencyMhz: null },
            gpus: [{ name: 'virtio', vendor: null, driver: 'virtio-pci', memoryBytes: null, address: '0000:00:01.0' }],
            memory: { totalBytes: 4, availableBytes: 3, swapTotalBytes: 0, swapFreeBytes: 0 },
            disks: [
                {
                    mountPoint: '/',
                    device: '/dev/vda1',
                    fileSystem: 'ext4',
                    kind: 'ssd',
                    totalBytes: 10,
                    availableBytes: 5,
                    readOnly: false,
                },
            ],
            network: [{ name: 'eth0', addresses: ['203.0.113.7/24'], receivedBytes: 1, transmittedBytes: 2 }],
            uptimeSecs: 60,
            bootTime: 1,
            loadAverage: [0.5, 0.25, 0],
            timezone: null,
            agentVersion: '1.0.0',
        };
        expect(parseHostInfo(host)).toEqual(host);
        expect(() => parseHostInfo({ ...host, loadAverage: [1, 2] })).toThrow(TypeError);
        expect(() => parseHostInfo({ ...host, disks: [{ ...host.disks[0], kind: 'nvme' }] })).toThrow(TypeError);
    });

    test('un agente sin límites en el estado sigue funcionando', () => {
        const status = {
            name: 'api',
            unit: 'blacksmith@api.service',
            loadState: 'loaded',
            activeState: 'active',
            subState: 'running',
            pid: 1,
            deployed: true,
            release: null,
            previousRelease: null,
            resources: {
                memoryBytes: 1,
                memoryPeakBytes: null,
                memoryLimitBytes: null,
                swapBytes: null,
                cpuPercent: null,
                tasks: null,
                restarts: null,
                uptimeSecs: null,
            },
            host: { cpuCores: 1, memoryBytes: 1, swapBytes: 0 },
        };
        const parsed = parseServiceStatus(status);
        expect(parsed.resources.cpuLimitPercent).toBeNull();
        expect(parsed.resources.tasksLimit).toBeNull();
    });
});
