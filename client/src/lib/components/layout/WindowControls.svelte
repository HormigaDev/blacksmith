<!--
    Botones de la ventana (minimizar, maximizar/restaurar, cerrar) para la ventana sin marco
    (`decorations: false` en tauri.conf.json). Llaman a los comandos de ventana de Tauri; fuera de
    Tauri (navegador) no se muestran, salvo con `preview` (Workbench).
-->
<script lang="ts">
    import CopyIcon from '@lucide/svelte/icons/copy';
    import MinusIcon from '@lucide/svelte/icons/minus';
    import SquareIcon from '@lucide/svelte/icons/square';
    import XIcon from '@lucide/svelte/icons/x';
    import { isTauri } from '@tauri-apps/api/core';
    import { getCurrentWindow } from '@tauri-apps/api/window';

    import { t } from '$lib/i18n';

    interface Props {
        /** Mostrar aunque no haya Tauri (solo para el Workbench). */
        preview?: boolean;
    }

    let { preview = false }: Props = $props();

    const native = isTauri();
    let maximized = $state(false);

    $effect(() => {
        if (!native) return;
        const appWindow = getCurrentWindow();
        let unlisten: (() => void) | undefined;
        let disposed = false;
        const sync = async () => {
            maximized = await appWindow.isMaximized();
        };
        void sync();
        void appWindow
            .onResized(() => void sync())
            .then((stop) => {
                if (disposed) stop();
                else unlisten = stop;
            });
        return () => {
            disposed = true;
            unlisten?.();
        };
    });

    function run(action: (appWindow: ReturnType<typeof getCurrentWindow>) => Promise<void>): void {
        if (!native) return;
        action(getCurrentWindow()).catch((error: unknown) => console.warn('Acción de ventana fallida', error));
    }
</script>

{#if native || preview}
    <div class="controls" role="group" aria-label={t('window.controls')}>
        <button
            type="button"
            class="control"
            aria-label={t('window.minimize')}
            onclick={() => run((appWindow) => appWindow.minimize())}
        >
            <MinusIcon />
        </button>
        <button
            type="button"
            class="control"
            aria-label={maximized ? t('window.restore') : t('window.maximize')}
            onclick={() => run((appWindow) => appWindow.toggleMaximize())}
        >
            {#if maximized}<CopyIcon />{:else}<SquareIcon />{/if}
        </button>
        <button
            type="button"
            class="control close"
            aria-label={t('window.close')}
            onclick={() => run((appWindow) => appWindow.close())}
        >
            <XIcon />
        </button>
    </div>
{/if}

<style>
    .controls {
        display: flex;
        align-self: stretch;
    }

    .control {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: var(--layout-header-height);
        padding: var(--space-0);
        color: var(--color-text-muted);
        background-color: transparent;
        border: none;
        transition:
            background-color var(--duration-fast) var(--easing-standard),
            color var(--duration-fast) var(--easing-standard);
    }

    .control :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
    }

    .control:hover {
        color: var(--color-text);
        background-color: var(--color-surface-hover);
    }

    .control:active {
        background-color: var(--color-surface-active);
    }

    .close:hover,
    .close:active {
        color: var(--color-on-danger);
        background-color: var(--color-danger);
    }

    .control:focus-visible {
        outline: var(--focus-ring-width) solid var(--color-focus-ring);
        outline-offset: calc(-1 * var(--focus-ring-width));
    }
</style>
