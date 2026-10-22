import { faGithub } from '@fortawesome/free-brands-svg-icons';
import { faServer } from '@fortawesome/free-solid-svg-icons';
import { expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import FaIcon from '../FaIcon.svelte';

test('decorativo por defecto: oculto a lectores de pantalla', async () => {
    const screen = await render(FaIcon, { icon: faServer });
    const svg = screen.container.querySelector('svg');
    expect(svg?.getAttribute('aria-hidden')).toBe('true');
    expect(svg?.getAttribute('viewBox')).toBe(`0 0 ${faServer.icon[0]} ${faServer.icon[1]}`);
});

test('con label es una imagen accesible', async () => {
    await render(FaIcon, { icon: faGithub, label: 'GitHub' });
    await expect.element(page.getByRole('img', { name: 'GitHub' })).toBeVisible();
});

test('toma el tamaño de los tokens, no de la definición del ícono', async () => {
    const screen = await render(FaIcon, { icon: faServer, size: 'lg' });
    const svg = screen.container.querySelector('svg') as SVGSVGElement;
    const expected = getComputedStyle(document.documentElement).getPropertyValue('--icon-size-lg').trim();
    expect(`${svg.getBoundingClientRect().height / 16}rem`).toBe(expected);
});
