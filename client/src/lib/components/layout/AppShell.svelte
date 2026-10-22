<!--
    Marco de la ventana principal: cabecera fija y área de contenido con scroll propio.

    La ventana no tiene marco nativo (`decorations: false`): la cabecera hace de barra de título.
    `data-tauri-drag-region="deep"` permite arrastrar la ventana desde cualquier punto de la cabecera
    que no sea un control interactivo; doble clic maximiza/restaura.
-->
<script lang="ts">
    import { asset } from '$app/paths';
    import type { Snippet } from 'svelte';

    import { APP_SHORT_NAME } from '$lib/app';
    import LocaleSwitcher from '$lib/components/locale/LocaleSwitcher.svelte';
    import ThemeSwitcher from '$lib/components/theme/ThemeSwitcher.svelte';

    import WindowControls from './WindowControls.svelte';

    let { children }: { children: Snippet } = $props();
</script>

<div class="shell">
    <header class="header" data-tauri-drag-region="deep">
        <span class="brand">
            <img class="logo" src={asset('/blacksmith-logo.png')} alt="" draggable="false" />
            {APP_SHORT_NAME}
        </span>
        <div class="actions">
            <LocaleSwitcher />
            <ThemeSwitcher />
        </div>
        <WindowControls />
    </header>

    <main class="content">
        {@render children()}
    </main>
</div>

<style>
    .shell {
        display: grid;
        grid-template-rows: var(--layout-header-height) minmax(0, 1fr);
        height: 100vh;
    }

    .header {
        display: flex;
        align-items: center;
        gap: var(--space-4);
        padding-left: var(--space-4);
        user-select: none;
        background-color: var(--color-surface);
        border-bottom: var(--border-width) solid var(--color-border);
    }

    .brand {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
        font-weight: var(--font-weight-semibold);
        color: var(--color-text);
    }

    /* Tamaño de ícono: cabe en la altura fija de la cabecera sin agrandarla. */
    .logo {
        width: var(--icon-size-lg);
        height: var(--icon-size-lg);
    }

    .actions {
        display: flex;
        margin-right: var(--space-2);
        margin-left: auto;
        align-items: center;
        gap: var(--space-4);
    }

    /* Bloque contenedor de los elementos absolutos (etiquetas ocultas, controles flotantes):
       sin él escapan del recorte y alargan el documento, que vuelve a hacer scroll entero. */
    .content {
        position: relative;
        overflow: auto;
        padding: var(--space-6);
    }
</style>
