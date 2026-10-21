// Grupo (dev): herramientas de desarrollo. No existen en un build de producción.
import { error } from '@sveltejs/kit';

import { dev } from '$app/environment';

export function load(): void {
    if (!dev) error(404);
}
