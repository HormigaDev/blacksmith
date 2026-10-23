<!--
    Marco de la ventana principal: barra lateral (marca, servidores, GitHub y preferencias) y
    contenido con scroll propio.

    El marco es FIJO: ocupa exactamente la ventana y nunca se desplaza; la barra de título y los
    controles de ventana siempre quedan a la vista. Solo hacen scroll `.content` y la lista de
    servidores (styles.md, «Marco fijo»).

    Compacto (ventana estrecha o móvil, styles.md «Tamaños de ventana»): la barra lateral pasa a
    ser un panel que se abre con el botón de menú de la barra de título. Se cierra al navegar, con
    Escape o al pulsar fuera, y devuelve el foco al botón.

    La ventana no tiene marco nativo (`decorations: false`): la barra superior del contenido y la
    cabecera de la barra lateral hacen de barra de título. `data-tauri-drag-region="deep"` permite
    arrastrar la ventana desde cualquier punto que no sea un control; doble clic maximiza/restaura.
-->
<script lang="ts">
    import LayoutGridIcon from '@lucide/svelte/icons/layout-grid';
    import MenuIcon from '@lucide/svelte/icons/menu';
    import XIcon from '@lucide/svelte/icons/x';
    import { tick, type Snippet } from 'svelte';

    import { asset } from '$app/paths';
    import { page } from '$app/state';
    import { APP_SHORT_NAME } from '$lib/app';
    import LocaleSwitcher from '$lib/components/locale/LocaleSwitcher.svelte';
    import ThemeSwitcher from '$lib/components/theme/ThemeSwitcher.svelte';
    import { t } from '$lib/i18n';
    import { servers } from '$lib/stores';

    import GitHubMenu from '../github/GitHubMenu.svelte';
    import Button from '../ui/Button.svelte';
    import ServerNav from '../servers/ServerNav.svelte';
    import WindowControls from './WindowControls.svelte';

    let { children }: { children: Snippet } = $props();

    const sidebarId = $props.id();
    let navOpen = $state(false);
    let sidebar = $state<HTMLElement>();
    let menuButton = $state<HTMLElement>();

    // Al navegar, el panel se cierra (solo importa en modo compacto).
    $effect(() => {
        void page.url.pathname;
        navOpen = false;
    });

    async function openNav(): Promise<void> {
        navOpen = true;
        await tick();
        sidebar?.querySelector<HTMLElement>('a, button')?.focus();
    }

    function closeNav(returnFocus = true): void {
        navOpen = false;
        if (returnFocus) menuButton?.querySelector('button')?.focus();
    }

    function onkeydown(event: KeyboardEvent): void {
        if (navOpen && event.key === 'Escape') closeNav();
    }

    function onpointerdown(event: PointerEvent): void {
        const target = event.target as Node;
        if (navOpen && !sidebar?.contains(target) && !menuButton?.contains(target)) closeNav(false);
    }
</script>

<svelte:window {onkeydown} {onpointerdown} />

<div class="shell">
    <aside class="sidebar" class:open={navOpen} id={sidebarId} bind:this={sidebar}>
        <div class="brand" data-tauri-drag-region="deep">
            <img class="logo" src={asset('/blacksmith-logo.png')} alt="" draggable="false" />
            {APP_SHORT_NAME}
        </div>
        <div class="navigation">
            <a class="home" href="/" aria-current={page.url.pathname === '/' ? 'page' : undefined}>
                <LayoutGridIcon />
                {t('servers.title')}
            </a>
            <ServerNav entries={servers.entries} currentPath={page.url.pathname} />
        </div>
        <div class="footer">
            <GitHubMenu />
            <div class="preferences">
                <LocaleSwitcher />
                <ThemeSwitcher />
            </div>
        </div>
    </aside>
    <!-- Solo visual: el cierre al pulsar fuera lo gestiona `onpointerdown`. -->
    <div class="backdrop" class:open={navOpen} aria-hidden="true"></div>

    <div class="main">
        <header class="titlebar" data-tauri-drag-region="deep">
            <div class="compact-brand" bind:this={menuButton}>
                <Button
                    variant="ghost"
                    size="sm"
                    iconOnly
                    aria-label={navOpen ? t('layout.navigation.close') : t('layout.navigation.open')}
                    aria-expanded={navOpen}
                    aria-controls={sidebarId}
                    onclick={() => (navOpen ? closeNav() : openNav())}
                >
                    {#if navOpen}<XIcon />{:else}<MenuIcon />{/if}
                </Button>
                <img class="logo" src={asset('/blacksmith-logo.png')} alt="" draggable="false" />
                {APP_SHORT_NAME}
            </div>
            <WindowControls />
        </header>
        <main class="content">
            <div class="page">
                {@render children()}
            </div>
        </main>
    </div>
</div>

<style>
    /* Fijado a la ventana y con la fila acotada: ningún contenido puede estirar el marco (en
       WebKitGTK una fila implícita `auto` crecía con la página y la desplazaba entera). */
    .shell {
        position: fixed;
        inset: 0;
        display: grid;
        grid-template-rows: minmax(0, 1fr);
        grid-template-columns: var(--layout-sidebar-width) minmax(0, 1fr);
        overflow: hidden;
    }

    .sidebar {
        display: flex;
        flex-direction: column;
        min-height: 0;
        overflow: hidden;
        background-color: var(--color-bg-subtle);
        border-right: var(--border-width) solid var(--color-border);
    }

    .brand {
        display: flex;
        flex-shrink: 0;
        align-items: center;
        gap: var(--space-2);
        height: var(--layout-header-height);
        padding: var(--space-0) var(--space-4);
        font-weight: var(--font-weight-semibold);
        letter-spacing: var(--letter-spacing-tight);
        color: var(--color-text);
        user-select: none;
    }

    /* Tamaño de ícono: cabe en la altura de la barra de título sin agrandarla. */
    .logo {
        width: var(--icon-size-lg);
        height: var(--icon-size-lg);
    }

    .navigation {
        display: flex;
        flex: 1;
        flex-direction: column;
        gap: var(--space-2);
        min-height: 0;
        padding: var(--space-2) var(--space-3);
        overflow-y: auto;
    }

    .home {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-2) var(--space-3);
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-medium);
        color: var(--color-text-muted);
        text-decoration: none;
        border-radius: var(--radius-md);
    }

    .home :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
    }

    .home:hover {
        color: var(--color-text);
        background-color: var(--color-surface-hover);
    }

    .home[aria-current='page'] {
        color: var(--color-text);
        background-color: var(--color-surface);
        box-shadow: var(--shadow-sm);
    }

    .home:focus-visible {
        outline: var(--focus-ring-width) solid var(--color-focus-ring);
        outline-offset: calc(-1 * var(--focus-ring-width));
    }

    .footer {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        padding: var(--space-3);
        border-top: var(--border-width) solid var(--color-border);
    }

    .preferences {
        display: grid;
        gap: var(--space-2);
    }

    .main {
        display: grid;
        grid-template-rows: var(--layout-header-height) minmax(0, 1fr);
        min-width: 0;
        min-height: 0;
        overflow: hidden;
    }

    .titlebar {
        display: flex;
        align-items: center;
        justify-content: flex-end;
        min-width: 0;
        user-select: none;
    }

    .compact-brand {
        display: none;
        flex: 1;
        align-items: center;
        gap: var(--space-2);
        min-width: 0;
        padding-inline-start: var(--space-2);
        font-weight: var(--font-weight-semibold);
        letter-spacing: var(--letter-spacing-tight);
        white-space: nowrap;
    }

    /* Bloque contenedor de los elementos absolutos (etiquetas ocultas, controles flotantes):
       sin él escapan del recorte y alargan el documento, que vuelve a hacer scroll entero. */
    .content {
        position: relative;
        min-height: 0;
        overflow: auto;
        overscroll-behavior: contain;
        padding: var(--space-2) var(--space-8) var(--space-8);
    }

    .page {
        max-width: var(--layout-content-max-width);
        margin: var(--space-0) auto;
    }

    /* Compacto (styles.md, «Tamaños de ventana»): una sola columna y la barra lateral como panel
       superpuesto. Oculto con `visibility`, que además lo saca del orden de tabulación. */
    .backdrop {
        display: none;
    }

    @media (max-width: 48rem) {
        .backdrop.open {
            position: fixed;
            inset: 0;
            z-index: var(--z-overlay);
            display: block;
            background-color: var(--color-overlay);
        }

        .shell {
            grid-template-columns: minmax(0, 1fr);
        }

        .sidebar {
            position: fixed;
            inset-block: 0;
            inset-inline-start: 0;
            z-index: var(--z-modal);
            width: min(var(--layout-sidebar-width), calc(100% - var(--space-12)));
            visibility: hidden;
            box-shadow: var(--shadow-lg);
            transform: translateX(-100%);
            transition:
                transform var(--duration-normal) var(--easing-standard),
                visibility var(--duration-normal) var(--easing-standard);
        }

        /* Al abrir, visible al instante (para poder enfocarlo); al cerrar, espera a la animación. */
        .sidebar.open {
            visibility: visible;
            transform: none;
            transition: transform var(--duration-normal) var(--easing-standard);
        }

        .compact-brand {
            display: flex;
        }

        .content {
            padding: var(--space-2) var(--space-4) var(--space-6);
        }
    }
</style>
