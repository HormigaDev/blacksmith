import { describe, expect, test } from 'vitest';

import { mockTauri } from '../../tests/tauri';
import { serviceStore } from './services.svelte';

const RUN = {
    id: 7,
    status: 'running',
    kind: 'deploy',
    release: 'r1800000000-000000000',
    branch: 'main',
    startedAt: 1,
    finishedAt: null,
    detail: null,
};

/** Agente simulado: responde al despliegue y registra el orden de las llamadas. */
function agent() {
    return mockTauri({
        github_share_with_server: () => ({ login: 'octocat', savedAt: 1_800_000_000 }),
        agent_request: (args) => {
            const request = args.request as { method: string; path: string };
            const ok = request.method === 'POST' && request.path === '/api/v1/services/api/deploy';
            return {
                status: ok ? 200 : 404,
                contentType: 'application/json',
                retryAfter: null,
                requestId: 'test',
                body: JSON.stringify(ok ? RUN : { code: 'NOT_FOUND', message: 'no' }),
            };
        },
    });
}

describe('serviceStore', () => {
    test('al desplegar, el servidor recibe antes la cuenta de GitHub conectada', async () => {
        const { calls } = agent();
        const service = serviceStore.service('local:4040', 'api');
        const run = await service.deploy('octocat');

        expect(run.id).toBe(7);
        expect(calls.map((call) => call.cmd)).toEqual(['github_share_with_server', 'agent_request']);
        expect(calls[0]?.args).toEqual({ serverId: 'local:4040' });
        expect(serviceStore.server('local:4040').github?.login).toBe('octocat');
        expect(service.runs?.[0]?.status).toBe('running');
    });

    test('sin cuenta de GitHub conectada despliega igual (repositorios públicos)', async () => {
        const { calls } = agent();
        await serviceStore.service('otro:4040', 'api').deploy(null);
        expect(calls.map((call) => call.cmd)).toEqual(['agent_request']);
    });
});
