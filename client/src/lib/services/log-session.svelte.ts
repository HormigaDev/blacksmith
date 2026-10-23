/**
 * Estado de un visor de logs: las entradas recibidas y en qué punto está el flujo. No sabe cómo
 * se conecta (se le pasa `follow`), así que se prueba sin Tauri.
 *
 * - Como mucho `MAX_LOG_LINES` entradas: un servicio muy hablador no hace crecer la memoria.
 * - Cada `start` invalida el anterior: un evento tardío de una sesión ya cerrada se ignora.
 */
import type { LogEndReason, LogEntry, LogEvent } from '$lib/agent';
import { describeError } from '$lib/http';

export const MAX_LOG_LINES = 2000;

export type LogStatus = 'idle' | 'connecting' | 'live' | 'ended' | 'error';

/** Entrada con un id estable para las listas con clave. */
export interface LogLine extends LogEntry {
    id: number;
}

export type FollowLogs = (onEvent: (event: LogEvent) => void) => Promise<() => Promise<void>>;

export class LogSession {
    lines = $state.raw<readonly LogLine[]>([]);
    status = $state<LogStatus>('idle');
    endReason = $state<LogEndReason | null>(null);
    /** Mensaje traducido si no se pudo conectar. */
    error = $state<string | null>(null);

    #follow: FollowLogs;
    #stop: (() => Promise<void>) | undefined;
    #generation = 0;
    #nextId = 0;

    constructor(follow: FollowLogs) {
        this.#follow = follow;
    }

    async start(): Promise<void> {
        const generation = ++this.#generation;
        this.lines = [];
        this.status = 'connecting';
        this.endReason = null;
        this.error = null;
        await this.#close();
        try {
            const stop = await this.#follow((event) => this.#receive(generation, event));
            if (generation !== this.#generation) {
                await stop();
                return;
            }
            this.#stop = stop;
            if (this.status === 'connecting') this.status = 'live';
        } catch (error) {
            if (generation !== this.#generation) return;
            this.status = 'error';
            this.error = describeError(error);
        }
    }

    async stop(): Promise<void> {
        this.#generation++;
        await this.#close();
        if (this.status === 'connecting' || this.status === 'live') this.status = 'idle';
    }

    #receive(generation: number, event: LogEvent): void {
        if (generation !== this.#generation) return;
        if (event.type === 'end') {
            this.status = 'ended';
            this.endReason = event.reason;
            this.#stop = undefined;
            return;
        }
        if (this.status === 'connecting') this.status = 'live';
        const { type: _type, ...entry } = event;
        const next = [...this.lines, { ...entry, id: this.#nextId++ }];
        this.lines = next.length > MAX_LOG_LINES ? next.slice(next.length - MAX_LOG_LINES) : next;
    }

    async #close(): Promise<void> {
        const stop = this.#stop;
        this.#stop = undefined;
        await stop?.();
    }
}
