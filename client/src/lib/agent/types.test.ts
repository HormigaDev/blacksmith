import { describe, expect, test } from 'vitest';

import {
    parseLogEvent,
    parseServiceDeployment,
    parseSite,
    parseServiceEnvironment,
    parseServiceList,
    parseServiceStatus,
    SERVICE_NAME_PATTERN,
} from './types';

describe('parseServiceList', () => {
    test('acepta el JSON del agente y rechaza formas inesperadas', () => {
        const service = { name: 'api', displayName: 'API', description: '', unit: 'bbel-api.service', createdAt: 1 };
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
        unit: 'bbel-api.service',
        loadState: 'loaded',
        activeState: 'active',
        subState: 'running',
        pid: 42,
        metrics: { rssBytes: 1024, rssMb: 0, cpuPercent: 1.5 },
        deployed: true,
        release: 'r1800000000-000000000',
        previousRelease: null,
    };
    test('acepta el JSON del agente', () => {
        expect(parseServiceStatus(status)).toEqual(status);
        expect(parseServiceStatus({ ...status, pid: null, metrics: null }).metrics).toBeNull();
    });
    test('rechaza estados desconocidos', () => {
        expect(() => parseServiceStatus({ ...status, activeState: 'bailando' })).toThrow(TypeError);
        expect(() => parseServiceStatus({ ...status, loadState: 'otro' })).toThrow(TypeError);
        expect(() => parseServiceStatus({ ...status, deployed: undefined })).toThrow(TypeError);
        expect(() => parseServiceStatus({ ...status, release: 1 })).toThrow(TypeError);
        expect(() => parseServiceStatus({ ...status, metrics: { rssBytes: '1' } })).toThrow(TypeError);
    });
});

describe('parseServiceDeployment y parseServiceEnvironment', () => {
    test('despliegue con y sin repositorio', () => {
        const deployment = {
            name: 'api',
            repository: { owner: 'o', name: 'r', branch: 'main' },
            access: 'account',
            workingDirectory: '/srv/bbel/api',
            setupCommands: ['npm ci'],
            startCommand: 'npm start',
            publishDir: null,
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
            tls: 'issuing',
            tlsDetail: null,
            configPath: '/var/lib/blacksmith/nginx/sites/web.conf',
            updatedAt: 1,
        };
        expect(parseSite(site)).toEqual(site);
        expect(parseSite(null)).toBeNull();
        expect(() => parseSite({ ...site, template: 'php' })).toThrow(TypeError);
        expect(() => parseSite({ ...site, tls: 'maybe' })).toThrow(TypeError);
    });

    test('variables del servicio', () => {
        const env = {
            name: 'api',
            unit: 'bbel-api.service',
            file: '/etc/bbel/env/bbel-api.env',
            loadedByUnit: true,
            globalLoadedByUnit: false,
            variables: [{ key: 'A', value: '1' }],
        };
        expect(parseServiceEnvironment(env)).toEqual(env);
        expect(() => parseServiceEnvironment({ ...env, variables: [{ key: 'A' }] })).toThrow(TypeError);
    });
});
