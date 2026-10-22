// Componentes con historias, descubiertos en el sistema de archivos al recoger los tests: un test
// por componente sin listas a mano. `toSlug` replica la de src/lib/workbench/stories.ts (que usa
// `import.meta.glob` y no se puede cargar fuera de Vite); stories.spec.ts verifica que coinciden.
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const COMPONENTS = join(import.meta.dirname, '..', '..', 'src', 'lib', 'components');

export function toSlug(value: string): string {
    return value
        .replace(/([a-z\d])([A-Z])/g, '$1-$2')
        .normalize('NFD')
        .replace(/[̀-ͯ]/g, '')
        .toLowerCase()
        .replace(/[^a-z\d]+/g, '-')
        .replace(/^-|-$/g, '');
}

export const STORY_COMPONENTS: readonly string[] = readdirSync(COMPONENTS, { recursive: true, encoding: 'utf8' })
    .filter((file) => file.endsWith('.stories.svelte'))
    .map((file) => toSlug(file.split(/[\\/]/).pop()!.replace('.stories.svelte', '')))
    .sort();

export const THEMES = ['light', 'dark'] as const;
