// Simulación del runtime de Tauri para tests de componente. En la app, `isTauri()` decide entre
// las APIs nativas y el modo navegador; aquí se activa a mano para probar la rama nativa.
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach } from 'vitest';
import { cleanup } from 'vitest-browser-svelte';

type Handler = (args: Record<string, unknown>) => unknown;

/** Activa Tauri simulado con un manejador por comando (`plugin:dialog|open`, …). */
export function mockTauri(handlers: Record<string, Handler>): { calls: { cmd: string; args: unknown }[] } {
    const calls: { cmd: string; args: unknown }[] = [];
    Object.assign(globalThis, { isTauri: true });
    Object.assign(window, {
        __TAURI_INTERNALS__: {
            ...(window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__,
            metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
        },
    });
    mockIPC(
        (cmd, args) => {
            calls.push({ cmd, args });
            const handler = handlers[cmd];
            if (handler) return handler((args ?? {}) as Record<string, unknown>);
            // Los comandos no previstos se ignoran (p. ej. setTheme de la ventana).
            return null;
        },
        { shouldMockEvents: true },
    );
    return { calls };
}

afterEach(() => {
    // Primero se desmontan los componentes: sus efectos aún usan el runtime simulado al limpiar.
    cleanup();
    clearMocks();
    Reflect.deleteProperty(globalThis, 'isTauri');
});
