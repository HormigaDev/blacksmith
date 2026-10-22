// Entorno de los tests de componente (navegador real): mismos estilos globales que la app, para
// que la visibilidad, el foco y el posicionamiento se comporten igual que en producción.
import '$lib/styles/global.css';

import { afterEach } from 'vitest';

afterEach(() => {
    // Cada test empieza sin preferencias guardadas (tema, idioma).
    localStorage.clear();
});
