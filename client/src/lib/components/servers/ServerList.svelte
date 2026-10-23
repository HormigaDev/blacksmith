<!--
    Lista o cuadrícula de servidores, reordenable.

    - Ratón / táctil: arrastrando el asa. Se usan eventos de puntero, no el drag & drop de HTML5:
      en Windows, Tauri lo desactiva dentro de la webview mientras el soltar archivos nativo (que
      usa el FilePicker) está activo.
    - Teclado: con el asa enfocada, flechas para mover una posición, Inicio y Fin para los
      extremos. Cada movimiento se anuncia a lectores de pantalla.

    `onMove` reordena al momento (respuesta inmediata); `onCommit` persiste el orden al terminar.
    Mientras se arrastra, una copia flotante (fantasma) sigue al puntero y el hueco del elemento
    queda marcado en su posición provisional.
-->
<script lang="ts">
    import GripVerticalIcon from '@lucide/svelte/icons/grip-vertical';
    import { tick } from 'svelte';
    import { flip } from 'svelte/animate';

    import { t } from '$lib/i18n';
    import { serverAddress, serverHref } from '$lib/servers/address';
    import type { ServerEntry, ServerView } from '$lib/servers/servers.svelte';

    import Button from '../ui/Button.svelte';
    import ServerCard from './ServerCard.svelte';

    interface Props {
        entries: readonly ServerEntry[];
        view: ServerView;
        onMove: (from: number, to: number) => void;
        onCommit: () => void | Promise<void>;
        onConnect: (id: string) => void | Promise<void>;
        onDisconnect: (id: string) => void | Promise<void>;
        onRemove: (id: string) => void | Promise<void>;
        onForgetHostKey: (id: string) => void | Promise<void>;
    }

    let { entries, view, onMove, onCommit, onConnect, onDisconnect, onRemove, onForgetHostKey }: Props = $props();

    const uid = $props.id();
    const instructionsId = `server-list-${uid}-instructions`;
    let container = $state<HTMLElement>();
    let draggingId = $state<string | null>(null);
    /** Fantasma: tamaño del elemento agarrado y su esquina superior izquierda en la ventana. */
    let ghost = $state<{ width: number; height: number; x: number; y: number } | null>(null);
    // Se busca en `entries` para que el fantasma refleje cambios de estado durante el arrastre.
    const ghostEntry = $derived(ghost && entries.find((entry) => entry.profile.id === draggingId));
    let announcement = $state('');

    // Duración del token (`--duration-normal`), que ya vale 0 con movimiento reducido.
    const flipDuration = $derived.by(() => {
        if (!container) return 0;
        return parseFloat(getComputedStyle(container).getPropertyValue('--duration-normal')) || 0;
    });

    const noop = () => {};

    interface Box {
        left: number;
        top: number;
        right: number;
        bottom: number;
    }

    /**
     * Caja de cada elemento según el layout, sin la transformación de `animate:flip`.
     * `getBoundingClientRect` devolvería la posición animada: durante la transición el puntero
     * volvería a "entrar" en el elemento recién desplazado y ambos se intercambiarían sin parar.
     */
    function layoutBoxes(): Box[] {
        if (!container) return [];
        const origin = container.getBoundingClientRect();
        return Array.from(container.children, (item) => {
            const element = item as HTMLElement;
            const left = origin.left + element.offsetLeft;
            const top = origin.top + element.offsetTop;
            return { left, top, right: left + element.offsetWidth, bottom: top + element.offsetHeight };
        });
    }

    /**
     * Posición a la que mover el elemento `from` con el puntero en (x, y), o -1 si no toca moverlo.
     * Solo se mueve al cruzar el centro del destino en el sentido del avance (histéresis): con
     * tamaños distintos, tras el cambio el puntero no queda sobre el otro y no rebota.
     */
    function targetIndex(from: number, x: number, y: number): number {
        const boxes = layoutBoxes();
        const origin = boxes[from];
        const to = boxes.findIndex((box) => x >= box.left && x <= box.right && y >= box.top && y <= box.bottom);
        const target = boxes[to];
        if (!origin || !target || to === from) return -1;
        const forward = to > from;
        // Misma fila (cuadrícula): manda el eje horizontal; si no, el vertical.
        const sameRow = Math.abs(target.top - origin.top) < 1;
        const position = sameRow ? x : y;
        const center = sameRow ? (target.left + target.right) / 2 : (target.top + target.bottom) / 2;
        return (forward ? position > center : position < center) ? to : -1;
    }

    function startDrag(event: PointerEvent, id: string): void {
        if (event.button !== 0) return;
        event.preventDefault();
        const item = (event.currentTarget as HTMLElement).closest('li');
        if (!item) return;
        const rect = item.getBoundingClientRect();
        // Distancia del puntero a la esquina: el fantasma se agarra por el mismo punto.
        const offsetX = event.clientX - rect.left;
        const offsetY = event.clientY - rect.top;
        draggingId = id;
        ghost = { width: rect.width, height: rect.height, x: rect.left, y: rect.top };
        let moved = false;

        // Se escucha en la ventana y no en el asa: al reordenar, el elemento se mueve en el DOM y
        // el navegador libera la captura del puntero, así que el asa dejaría de recibir eventos.
        const pointer = event.pointerId;
        const move = (next: PointerEvent) => {
            if (next.pointerId !== pointer) return;
            if (ghost) {
                ghost.x = next.clientX - offsetX;
                ghost.y = next.clientY - offsetY;
            }
            const from = entries.findIndex((entry) => entry.profile.id === id);
            const to = targetIndex(from, next.clientX, next.clientY);
            if (to !== -1) {
                onMove(from, to);
                moved = true;
            }
        };
        const end = (next: PointerEvent) => {
            if (next.pointerId !== pointer) return;
            window.removeEventListener('pointermove', move);
            window.removeEventListener('pointerup', end);
            window.removeEventListener('pointercancel', end);
            draggingId = null;
            ghost = null;
            if (moved) void onCommit();
        };
        window.addEventListener('pointermove', move);
        window.addEventListener('pointerup', end);
        window.addEventListener('pointercancel', end);
    }

    async function keyMove(event: KeyboardEvent, index: number): Promise<void> {
        const last = entries.length - 1;
        const targets: Record<string, number> = {
            ArrowUp: index - 1,
            ArrowLeft: index - 1,
            ArrowDown: index + 1,
            ArrowRight: index + 1,
            Home: 0,
            End: last,
        };
        const target = targets[event.key];
        if (target === undefined) return;
        event.preventDefault();
        const to = Math.min(Math.max(target, 0), last);
        const entry = entries[index];
        if (to === index || !entry) return;

        onMove(index, to);
        void onCommit();
        announcement = t('servers.reorder.moved', {
            address: serverAddress(entry.profile),
            position: to + 1,
            total: entries.length,
        });
        // Al mover el elemento en el DOM se pierde el foco: se devuelve a su asa.
        await tick();
        container?.querySelectorAll<HTMLElement>('[data-reorder-handle]')[to]?.focus();
    }
</script>

<ul class="servers {view}" bind:this={container}>
    {#each entries as entry, index (entry.profile.id)}
        <li class="item" class:dragging={draggingId === entry.profile.id} animate:flip={{ duration: flipDuration }}>
            <div class="slot">
                <ServerCard
                    {entry}
                    layout={view === 'grid' ? 'tile' : 'row'}
                    href={serverHref(entry.profile.id)}
                    onConnect={() => onConnect(entry.profile.id)}
                    onDisconnect={() => onDisconnect(entry.profile.id)}
                    onRemove={() => onRemove(entry.profile.id)}
                    onForgetHostKey={() => onForgetHostKey(entry.profile.id)}
                >
                    {#snippet handle()}
                        <Button
                            variant="ghost"
                            size="sm"
                            iconOnly
                            data-reorder-handle
                            style="cursor: {draggingId === entry.profile.id ? 'grabbing' : 'grab'}; touch-action: none;"
                            aria-label={t('servers.reorder.handle', { address: serverAddress(entry.profile) })}
                            aria-describedby={instructionsId}
                            onpointerdown={(event) => startDrag(event, entry.profile.id)}
                            onkeydown={(event) => keyMove(event, index)}
                        >
                            <GripVerticalIcon />
                        </Button>
                    {/snippet}
                </ServerCard>
            </div>
        </li>
    {/each}
</ul>

<!-- Solo visual: fuera del árbol de accesibilidad y sin interacción (el original sigue en la lista). -->
{#if ghost && ghostEntry}
    <div
        class="ghost"
        data-drag-ghost
        aria-hidden="true"
        inert
        style:--_width="{ghost.width}px"
        style:--_height="{ghost.height}px"
        style:--_x="{ghost.x}px"
        style:--_y="{ghost.y}px"
    >
        <ServerCard
            entry={ghostEntry}
            layout={view === 'grid' ? 'tile' : 'row'}
            onConnect={noop}
            onDisconnect={noop}
            onRemove={noop}
            onForgetHostKey={noop}
        >
            {#snippet handle()}
                <span class="ghost-grip"><GripVerticalIcon /></span>
            {/snippet}
        </ServerCard>
    </div>
{/if}

<p id={instructionsId} class="visually-hidden">{t('servers.reorder.instructions')}</p>
<p class="visually-hidden" aria-live="assertive">{announcement}</p>

<style>
    /* `position` hace de la lista el `offsetParent` de sus elementos (ver `layoutBoxes`). */
    .servers {
        position: relative;
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
        padding: var(--space-0);
        list-style: none;
    }

    .grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(var(--layout-card-min-width), 1fr));
        gap: var(--space-4);
    }

    .item {
        position: relative;
        border-radius: var(--radius-lg);
        transition: box-shadow var(--duration-fast) var(--easing-standard);
    }

    .slot {
        height: 100%;
    }

    /* Hueco del elemento arrastrado: conserva su tamaño, el contenido lo muestra el fantasma. */
    .dragging {
        background-color: var(--color-bg-subtle);
        outline: var(--border-width-strong) dashed var(--color-border-strong);
        outline-offset: calc(-1 * var(--border-width-strong));
    }

    /* `opacity` y no `visibility`: el original sigue siendo el que ven los lectores de pantalla. */
    .dragging .slot {
        opacity: 0;
    }

    .ghost {
        position: fixed;
        top: 0;
        left: 0;
        z-index: var(--z-overlay);
        width: var(--_width);
        height: var(--_height);
        border-radius: var(--radius-lg);
        box-shadow: var(--shadow-lg);
        cursor: grabbing;
        pointer-events: none;
        transform: translate3d(var(--_x), var(--_y), 0);
    }

    /* Mismo hueco que el asa real (botón `sm` de solo ícono) para que el fantasma no se descuadre. */
    .ghost-grip {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: var(--control-height-sm);
        height: var(--control-height-sm);
        color: var(--color-text-muted);
    }

    .ghost-grip :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
    }

    .visually-hidden {
        position: absolute;
        width: 1px;
        height: 1px;
        margin: calc(-1 * var(--space-px));
        padding: var(--space-0);
        overflow: hidden;
        clip: rect(0 0 0 0);
        white-space: nowrap;
        border: 0;
    }
</style>
