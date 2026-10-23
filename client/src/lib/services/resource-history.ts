/**
 * Historial de consumo para las tendencias del resumen de un servicio: la interfaz guarda CPU y
 * memoria de cada lectura de estado (el agente no guarda historia).
 */
import type { ServiceStatus } from '$lib/agent';

/** Lecturas que se conservan (6 min con una lectura cada 3 s). */
export const HISTORY_SAMPLES = 120;

export interface ResourceSample {
    /** Milisegundos Unix. */
    at: number;
    /** 100 = un núcleo; `null` si no se conoce (p. ej. parado: la tendencia se corta). */
    cpu: number | null;
    memory: number | null;
}

/** Añade la lectura de `status` al final y descarta las más antiguas. */
export function appendSample(
    history: readonly ResourceSample[],
    status: ServiceStatus,
    at: number = Date.now(),
): ResourceSample[] {
    const { cpuPercent, memoryBytes } = status.resources;
    return [...history, { at, cpu: cpuPercent, memory: memoryBytes }].slice(-HISTORY_SAMPLES);
}
