import { describe, expect, test } from 'vitest';

import type { ServiceStatus } from '$lib/agent';

import { appendSample, HISTORY_SAMPLES } from './resource-history';

const status = (cpu: number | null, memory: number | null) =>
    ({ resources: { cpuPercent: cpu, memoryBytes: memory } }) as ServiceStatus;

describe('appendSample', () => {
    test('añade al final y conserva solo las últimas', () => {
        let history = appendSample([], status(1, 10), 1);
        history = appendSample(history, status(null, null), 2);
        expect(history).toEqual([
            { at: 1, cpu: 1, memory: 10 },
            { at: 2, cpu: null, memory: null },
        ]);
        for (let at = 3; at < HISTORY_SAMPLES + 10; at++) history = appendSample(history, status(at, at), at);
        expect(history).toHaveLength(HISTORY_SAMPLES);
        expect(history.at(-1)?.at).toBe(HISTORY_SAMPLES + 9);
    });
});
