import { describe, expect, test, vi } from 'vitest';

import type { LogEvent } from '$lib/agent';
import { TunnelError } from '$lib/ssh';

import { LogSession, MAX_LOG_LINES } from './log-session.svelte';

const entry = (message: string): LogEvent => ({
    type: 'entry',
    timestampUs: 1,
    priority: 6,
    pid: null,
    identifier: null,
    message,
});

/** `follow` falso: guarda el emisor para enviar eventos desde el test. */
function fakeFollow() {
    const emitters: ((event: LogEvent) => void)[] = [];
    const stops = vi.fn(async () => {});
    const follow = vi.fn(async (onEvent: (event: LogEvent) => void) => {
        emitters.push(onEvent);
        return stops;
    });
    return { follow, emitters, stops, emit: (event: LogEvent) => emitters.at(-1)?.(event) };
}

describe('LogSession', () => {
    test('conecta, recibe entradas y termina con su motivo', async () => {
        const fake = fakeFollow();
        const session = new LogSession(fake.follow);
        const started = session.start();
        expect(session.status).toBe('connecting');
        await started;
        expect(session.status).toBe('live');

        fake.emit(entry('uno'));
        fake.emit(entry('dos'));
        expect(session.lines.map((line) => line.message)).toEqual(['uno', 'dos']);
        expect(new Set(session.lines.map((line) => line.id)).size).toBe(2);

        fake.emit({ type: 'end', reason: 'agentShutdown', detail: null });
        expect(session.status).toBe('ended');
        expect(session.endReason).toBe('agentShutdown');
    });

    test('conserva solo las últimas entradas', async () => {
        const fake = fakeFollow();
        const session = new LogSession(fake.follow);
        await session.start();
        for (let i = 0; i < MAX_LOG_LINES + 5; i++) fake.emit(entry(String(i)));
        expect(session.lines).toHaveLength(MAX_LOG_LINES);
        expect(session.lines[0]?.message).toBe('5');
    });

    test('un rechazo del agente queda como error traducido', async () => {
        const session = new LogSession(async () => {
            throw new TunnelError('AGENT_REJECTED', 'x', { status: 503, agentCode: 'LOG_STREAM_LIMIT' });
        });
        await session.start();
        expect(session.status).toBe('error');
        expect(session.error).toContain('demasiados visores');
    });

    test('parar cierra el flujo e ignora eventos tardíos', async () => {
        const fake = fakeFollow();
        const session = new LogSession(fake.follow);
        await session.start();
        const lateEmitter = fake.emitters[0]!;
        await session.stop();
        expect(fake.stops).toHaveBeenCalledOnce();
        expect(session.status).toBe('idle');
        lateEmitter(entry('tarde'));
        expect(session.lines).toEqual([]);
    });

    test('reiniciar cierra la sesión anterior y empieza de cero', async () => {
        const fake = fakeFollow();
        const session = new LogSession(fake.follow);
        await session.start();
        fake.emit(entry('vieja'));
        await session.start();
        expect(fake.stops).toHaveBeenCalledOnce();
        expect(session.lines).toEqual([]);
        fake.emitters[0]!(entry('de la anterior'));
        fake.emit(entry('nueva'));
        expect(session.lines.map((line) => line.message)).toEqual(['nueva']);
    });
});
