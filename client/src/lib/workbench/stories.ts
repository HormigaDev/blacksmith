/**
 * Registro de historias del Workbench: todos los `*.stories.svelte` de src/lib/components.
 *
 * El `import.meta.glob` va dentro de `import.meta.env.DEV`: en un build de producción la rama se
 * elimina y ninguna historia llega a los chunks de la app.
 */
import type { Component } from 'svelte';

export interface StoryMeta {
    /** Nombre del componente tal como aparece en el menú. */
    title: string;
    /** Sección del menú: Acciones, Formularios, Superposiciones… */
    group: string;
    description?: string;
}

export interface StoryEntry {
    /** Identificador en la URL: `Input.stories.svelte` → `input`. */
    slug: string;
    path: string;
    load: () => Promise<{ default: Component; meta: StoryMeta }>;
}

type Loader = () => Promise<{ default: Component; meta: StoryMeta }>;

const modules: Record<string, Loader> = import.meta.env.DEV
    ? import.meta.glob<{ default: Component; meta: StoryMeta }>('/src/lib/components/**/*.stories.svelte')
    : {};

/** Orden del menú; los grupos no listados van al final, alfabéticamente. */
export const GROUP_ORDER = [
    'Acciones',
    'Formularios',
    'Selección',
    'Superposiciones',
    'Estado',
    'Íconos',
    'Navegación',
    'Servidores',
    'Servicios',
    'GitHub',
];

export function toSlug(value: string): string {
    return value
        .replace(/([a-z\d])([A-Z])/g, '$1-$2')
        .normalize('NFD')
        .replace(/[̀-ͯ]/g, '')
        .toLowerCase()
        .replace(/[^a-z\d]+/g, '-')
        .replace(/^-|-$/g, '');
}

export const STORIES: readonly StoryEntry[] = Object.entries(modules)
    .map(([path, load]) => ({
        slug: toSlug(path.split('/').pop()?.replace('.stories.svelte', '') ?? path),
        path,
        load,
    }))
    .sort((a, b) => a.slug.localeCompare(b.slug));

export function findStory(slug: string): StoryEntry | undefined {
    return STORIES.find((story) => story.slug === slug);
}
