import { tick } from 'svelte';
import { describe, expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import type { ServerEntry, ServerView } from '$lib/servers/servers.svelte';

import ServerList from './ServerList.svelte';

const entry = (host: string): ServerEntry => ({
    profile: { id: `deploy@${host}:22`, kind: 'ssh', host, port: 22, username: 'deploy' },
    status: 'disconnected',
});

function setup(view: ServerView = 'list') {
    const onCommit = vi.fn();
    const props = $state({
        entries: [entry('uno.example.com'), entry('dos.example.com'), entry('tres.example.com')],
        view,
        onMove(from: number, to: number) {
            const next = [...props.entries];
            const [moved] = next.splice(from, 1);
            if (moved) next.splice(to, 0, moved);
            props.entries = next;
        },
        onCommit,
        onConnect: vi.fn(),
        onDisconnect: vi.fn(),
        onRemove: vi.fn(),
        onForgetHostKey: vi.fn(),
    });
    const order = () => props.entries.map((item) => item.profile.host);
    return { props, order, onCommit, render: () => render(ServerList, props) };
}

/** Caja de layout del elemento `index` (sin transformaciones de la animación). */
function itemBox(index: number) {
    const item = document.querySelectorAll<HTMLElement>('li')[index]!;
    const list = item.parentElement!.getBoundingClientRect();
    return { left: list.left + item.offsetLeft, top: list.top + item.offsetTop, height: item.offsetHeight };
}

/** Arrastre con eventos de puntero sintéticos (la lista los escucha en `window`). */
function pointer(target: Element, clientX = 0, clientY = 0) {
    const init = { pointerId: 1, button: 0, bubbles: true };
    target.dispatchEvent(new PointerEvent('pointerdown', { ...init, clientX, clientY }));
    return {
        move: (clientX: number, clientY: number) =>
            window.dispatchEvent(new PointerEvent('pointermove', { ...init, clientX, clientY })),
        up: () => window.dispatchEvent(new PointerEvent('pointerup', init)),
    };
}

const handle = (host: string) => page.getByRole('button', { name: `Mover deploy@${host}` });

describe('ServerList', () => {
    test('teclado: las flechas mueven, se anuncia y el foco sigue en el asa', async () => {
        const list = setup();
        await list.render();
        await expect.element(handle('uno.example.com')).toHaveAccessibleDescription(/flechas del teclado/);
        handle('uno.example.com').element().focus();
        await userEvent.keyboard('{ArrowDown}');
        expect(list.order()).toEqual(['dos.example.com', 'uno.example.com', 'tres.example.com']);
        expect(list.onCommit).toHaveBeenCalledOnce();
        await expect.element(page.getByText('deploy@uno.example.com: posición 2 de 3')).toBeInTheDocument();
        await expect.element(handle('uno.example.com')).toHaveFocus();
    });

    test('teclado: Fin e Inicio llevan a los extremos; en el borde no hace nada', async () => {
        const list = setup();
        await list.render();
        handle('uno.example.com').element().focus();
        await userEvent.keyboard('{ArrowUp}');
        expect(list.onCommit).not.toHaveBeenCalled();
        await userEvent.keyboard('{End}');
        expect(list.order()).toEqual(['dos.example.com', 'tres.example.com', 'uno.example.com']);
        await expect.element(handle('uno.example.com')).toHaveFocus();
        await userEvent.keyboard('{Home}');
        expect(list.order()).toEqual(['uno.example.com', 'dos.example.com', 'tres.example.com']);
        expect(list.onCommit).toHaveBeenCalledTimes(2);
    });

    test('puntero: mueve al cruzar el centro del destino y guarda una vez al soltar', async () => {
        const list = setup();
        await list.render();
        const drag = pointer(handle('uno.example.com').element());
        const dos = itemBox(1);
        drag.move(dos.left + 10, dos.top + 2);
        expect(list.order()).toEqual(['uno.example.com', 'dos.example.com', 'tres.example.com']);
        drag.move(dos.left + 10, dos.top + dos.height * 0.75);
        expect(list.order()).toEqual(['dos.example.com', 'uno.example.com', 'tres.example.com']);
        expect(list.onCommit).not.toHaveBeenCalled();
        drag.up();
        expect(list.onCommit).toHaveBeenCalledOnce();
    });

    test('puntero: un arrastre continuo no rebota entre dos tarjetas de distinta altura', async () => {
        const list = setup();
        list.props.entries[1] = {
            ...list.props.entries[1]!,
            status: 'error',
            error: 'Un detalle largo que hace la tarjeta más alta que las demás.',
        };
        await list.render();
        const moves = vi.spyOn(list.props, 'onMove');
        const drag = pointer(handle('uno.example.com').element());
        const uno = itemBox(0);
        const dos = itemBox(1);
        // Barrido gradual, como un arrastre real: del centro del propio elemento al 75 % del destino.
        // Cada paso espera a que Svelte actualice el DOM, como entre eventos reales.
        for (let y = uno.top + uno.height / 2; y <= dos.top + dos.height * 0.75; y += 2) {
            drag.move(dos.left + 10, y);
            await tick();
        }
        drag.up();
        expect(moves).toHaveBeenCalledOnce();
        expect(list.order()).toEqual(['dos.example.com', 'uno.example.com', 'tres.example.com']);
    });

    test('puntero: un fantasma sigue al puntero y desaparece al soltar', async () => {
        const list = setup();
        await list.render();
        const grip = handle('uno.example.com').element();
        const start = grip.getBoundingClientRect();
        const drag = pointer(grip, start.left + 4, start.top + 4);
        await tick();
        const ghost = () => document.querySelector<HTMLElement>('[data-drag-ghost]');
        expect(ghost()).not.toBeNull();
        // Solo visual: el lector de pantalla sigue viendo un único asa por servidor.
        expect(ghost()?.getAttribute('aria-hidden')).toBe('true');
        expect(page.getByRole('button', { name: 'Mover deploy@uno.example.com' }).elements()).toHaveLength(1);

        const before = ghost()!.getBoundingClientRect();
        drag.move(start.left + 4 + 30, start.top + 4 + 12);
        await tick();
        const after = ghost()!.getBoundingClientRect();
        expect(after.left - before.left).toBeCloseTo(30);
        expect(after.top - before.top).toBeCloseTo(12);

        drag.up();
        await tick();
        expect(ghost()).toBeNull();
    });

    test('cada servidor enlaza a su página, con el id codificado', async () => {
        const list = setup();
        await list.render();
        await expect
            .element(page.getByRole('link', { name: 'deploy@uno.example.com' }))
            .toHaveAttribute('href', '/servers/deploy%40uno.example.com%3A22');
    });

    test('cuadrícula: los servidores se muestran como tarjetas', async () => {
        const list = setup('grid');
        await list.render();
        await expect.element(page.getByRole('list')).toHaveClass('grid');
        expect(page.getByRole('listitem').elements()).toHaveLength(3);
    });
});
