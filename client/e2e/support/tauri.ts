/**
 * Runtime de Tauri simulado para E2E. La app corre en un navegador normal; esto instala, antes de
 * que cargue, lo mismo que inyecta Tauri (`isTauri`, `__TAURI_INTERNALS__`) para probar la rama
 * nativa: comandos (`invoke`) con respuestas predefinidas y eventos nativos (p. ej. soltar archivos).
 */
import type { Page } from '@playwright/test';

export interface TauriCall {
    cmd: string;
    args: Record<string, unknown> | undefined;
}

/** Respuesta del agente para `agent_request` (lo que devuelve el comando de Rust). */
export function agentJson(body: unknown, status = 200) {
    return { status, contentType: 'application/json', retryAfter: null, requestId: 'e2e', body: JSON.stringify(body) };
}

/** Respuesta por comando, p. ej. `{ 'plugin:dialog|open': '/home/ana/key.pem' }`. */
export type TauriResponses = Record<string, unknown>;

/** Respuestas por defecto de los comandos de la app: un equipo sin servidores configurados. */
const DEFAULT_RESPONSES: TauriResponses = { servers_list: [] };

export async function mockTauri(page: Page, overrides: TauriResponses = {}): Promise<void> {
    const responses = { ...DEFAULT_RESPONSES, ...overrides };
    await page.addInitScript((responses: TauriResponses) => {
        type Callback = (data: unknown) => unknown;
        const callbacks = new Map<number, Callback>();
        const listeners = new Map<string, number[]>();
        const calls: { cmd: string; args: unknown }[] = [];
        let next = 1;

        const internals = {
            metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
            callbacks,
            transformCallback(callback: Callback, once = false) {
                const id = next++;
                callbacks.set(id, (data) => {
                    if (once) callbacks.delete(id);
                    return callback?.(data);
                });
                return id;
            },
            unregisterCallback: (id: number) => callbacks.delete(id),
            runCallback: (id: number, data: unknown) => callbacks.get(id)?.(data),
            convertFileSrc: (path: string) => path,
            async invoke(cmd: string, args?: Record<string, unknown>) {
                calls.push({ cmd, args });
                if (cmd === 'plugin:event|listen') {
                    const event = String(args?.event);
                    listeners.set(event, [...(listeners.get(event) ?? []), Number(args?.handler)]);
                    return args?.handler;
                }
                if (cmd === 'plugin:event|unlisten') return null;
                // `agent_request` puede responder según la ruta de la API: `{ byPath: { '/api/v1/…': … } }`.
                const response = cmd in responses ? responses[cmd] : null;
                if (
                    cmd === 'agent_request' &&
                    typeof response === 'object' &&
                    response !== null &&
                    'byPath' in response
                ) {
                    const request = (args as { request?: { method?: string; path?: string } } | undefined)?.request;
                    const byPath = (response as { byPath: Record<string, unknown> }).byPath;
                    return byPath[`${request?.method} ${request?.path}`] ?? byPath[request?.path ?? ''] ?? null;
                }
                return response;
            },
        };

        Object.assign(window, {
            isTauri: true,
            __TAURI_INTERNALS__: internals,
            __TAURI_EVENT_PLUGIN_INTERNALS__: {
                unregisterListener: (_event: string, id: number) => callbacks.delete(id),
            },
            __E2E_TAURI__: {
                calls,
                emit(event: string, payload: unknown) {
                    for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id, payload });
                },
            },
        });
    }, responses);
}

/** Comandos que la app ha enviado a Tauri hasta ahora. */
export function tauriCalls(page: Page): Promise<TauriCall[]> {
    return page.evaluate(() => (window as unknown as { __E2E_TAURI__: { calls: TauriCall[] } }).__E2E_TAURI__.calls);
}

/** Emite un evento nativo, como si viniera de Rust (`tauri://drag-drop`, …). */
export function emitTauriEvent(page: Page, event: string, payload: unknown): Promise<void> {
    return page.evaluate(
        ([name, data]) =>
            (window as unknown as { __E2E_TAURI__: { emit: (e: string, p: unknown) => void } }).__E2E_TAURI__.emit(
                name as string,
                data,
            ),
        [event, payload] as const,
    );
}
