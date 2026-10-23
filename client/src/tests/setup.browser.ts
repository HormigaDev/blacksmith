// Entorno de los tests de componente (navegador real): mismos estilos globales que la app, para
// que la visibilidad, el foco y el posicionamiento se comporten igual que en producción.
import '$lib/styles/global.css';

import { afterEach } from 'vitest';

// En la app, el documento no se desplaza (marco fijo: styles.md §5) y los componentes viven dentro
// de la zona de contenido, que sí hace scroll. Aquí se montan sueltos: el documento hace de esa
// zona, o un componente más alto que el iframe quedaría fuera de alcance.
const scrollable = document.createElement('style');
scrollable.textContent = 'html, body { overflow: auto !important; }';
document.head.append(scrollable);

afterEach(() => {
    // Cada test empieza sin preferencias guardadas (tema, idioma).
    localStorage.clear();
});
