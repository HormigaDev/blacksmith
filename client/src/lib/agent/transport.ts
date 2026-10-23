/**
 * `fetch` que se ejecuta en Rust (comando `agent_request`, client/src-tauri/src/agent_api.rs).
 *
 * La webview no puede hablar con el agente: la CSP solo permite IPC y el agente rechaza cualquier
 * petición con `Origin`. Este transporte tiene la firma de `fetch` para usarlo con
 * `createHttpClient`, pero solo envía a Rust el servidor y la ruta: la dirección (túnel o agente
 * local) la resuelve Rust, y la URL que recibe aquí solo sirve para sacar la ruta.
 */
import { invoke } from '@tauri-apps/api/core';

import { toTunnelError } from '$lib/ssh';

/** Base de los clientes del agente. El host no se usa: Rust conoce la dirección real. */
export const AGENT_BASE_URL = 'http://agent.invalid/api/v1/';

interface AgentResponse {
    status: number;
    contentType: string | null;
    retryAfter: string | null;
    requestId: string | null;
    body: string;
}

/** Estados sin cuerpo según Fetch: `new Response(cuerpo, …)` falla con ellos. */
const NULL_BODY_STATUS = new Set([101, 103, 204, 205, 304]);

export function agentFetch(serverId: string): typeof fetch {
    return async (input, init = {}) => {
        const url = new URL(input instanceof Request ? input.url : String(input));
        const headers = new Headers(init.headers);
        const body = init.body;
        if (body !== undefined && body !== null && typeof body !== 'string') {
            throw new TypeError('el agente solo acepta cuerpos JSON en texto');
        }
        if (init.signal?.aborted) throw new DOMException('petición cancelada', 'AbortError');
        let response: AgentResponse;
        try {
            response = await invoke<AgentResponse>('agent_request', {
                request: {
                    serverId,
                    method: (init.method ?? 'GET').toUpperCase(),
                    path: `${url.pathname}${url.search}`,
                    body: body ?? undefined,
                    requestId: headers.get('x-request-id') ?? undefined,
                },
            });
        } catch (error) {
            // El cliente HTTP lo convierte en `NetworkError` y conserva esto como `cause`.
            throw toTunnelError(error);
        }
        const responseHeaders = new Headers();
        if (response.contentType) responseHeaders.set('content-type', response.contentType);
        if (response.retryAfter) responseHeaders.set('retry-after', response.retryAfter);
        if (response.requestId) responseHeaders.set('x-request-id', response.requestId);
        return new Response(NULL_BODY_STATUS.has(response.status) ? null : response.body, {
            status: response.status,
            headers: responseHeaders,
        });
    };
}
