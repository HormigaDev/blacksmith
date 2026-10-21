# Tests y Workbench del cliente

Lectura obligatoria antes de crear o cambiar un componente o una pantalla. Un cambio no está
terminado sin sus tests y, si es un componente, sin sus historias.

## 1. Comandos

| Comando                   | Qué hace                                                                |
| ------------------------- | ----------------------------------------------------------------------- |
| `pnpm workbench`          | Abre el **Workbench** en una ventana de Tauri (APIs nativas reales).    |
| `pnpm workbench:web`      | El mismo Workbench en el navegador (más rápido; sin APIs nativas).      |
| `pnpm test`               | Vitest: unitarios + componentes. Lo ejecuta `pnpm check`.               |
| `pnpm test:watch`         | Vitest en modo observación.                                             |
| `pnpm test:unit`          | Solo unitarios (Node).                                                  |
| `pnpm test:component`     | Solo componentes (Chromium real).                                       |
| `pnpm test:e2e`           | Playwright: app (Chromium + WebKit) y Workbench (humo + accesibilidad). |
| `pnpm test:visual`        | Regresión visual de todas las historias, ambos temas.                   |
| `pnpm test:visual:update` | Regenera las capturas de referencia tras un cambio visual intencionado. |
| `pnpm test:all`           | `test` + `test:e2e`.                                                    |

Requisitos: `pnpm exec playwright install chromium webkit`. Playwright usa 2 navegadores en
paralelo (`E2E_WORKERS` para cambiarlo): sin límite, con el editor abierto, se llegó a agotar la
memoria. En Linux, WebKit además necesita
`sudo apt-get install libevent-2.1-7t64 libgstreamer-plugins-bad1.0-0`; sin ellas,
`E2E_WEBKIT=0 pnpm test:e2e` omite WebKit (no en CI).

## 2. Niveles y dónde va cada test

| Nivel         | Herramienta                            | Archivo                          | Para qué                                                    |
| ------------- | -------------------------------------- | -------------------------------- | ----------------------------------------------------------- |
| Unitario      | Vitest (Node)                          | `src/**/*.test.ts`               | Lógica pura: HTTP, i18n, formatos, estado (`*.svelte.ts`).  |
| Componente    | Vitest Browser + Playwright (Chromium) | `src/**/*.svelte.test.ts`        | Un componente aislado: interacción, ARIA, eventos, props.   |
| E2E app       | Playwright (Chromium y WebKit)         | `e2e/app/*.spec.ts`              | Flujos de la app compilada, como la usa el usuario.         |
| Accesibilidad | Playwright + axe-core                  | `e2e/workbench/a11y.spec.ts`     | WCAG 2.1 A/AA (incluido contraste) de cada historia y tema. |
| Visual        | Playwright `toHaveScreenshot`          | `e2e/workbench/*.visual.spec.ts` | Que ningún cambio altere el aspecto sin querer.             |

Dónde va cada archivo: ver §5. Los de Playwright, siempre en `e2e/`.

¿Por qué estas herramientas?

- **Vitest, no Jest**: usa la misma configuración de Vite que la app (alias, plugin de Svelte,
  runes) sin transformaciones aparte, y es mucho más rápido.
- **Navegador real para componentes**, no jsdom: bits-ui depende de eventos de puntero, foco y
  posicionamiento que jsdom no simula bien. Un test que pasa en jsdom y falla en la app no sirve.
- **Chromium y WebKit en E2E**: son los dos motores reales de Tauri (WebView2 en Windows,
  WebKitGTK en Linux).

## 3. Cómo escribir tests

### Componentes

```ts
import { expect, test, vi } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import Select from './Select.svelte';

test('elige una opción', async () => {
    const onValueChange = vi.fn();
    const screen = await render(Select, { label: 'Unidad', options, onValueChange });
    await screen.getByRole('button', { name: 'Unidad' }).click();
    await page.getByRole('option', { name: 'api.service' }).click(); // los portales van a `page`
    expect(onValueChange).toHaveBeenCalledWith('api');
});
```

- Localizar **por rol y nombre accesible** (`getByRole('button', { name })`), como lo haría un
  lector de pantalla. Si no se puede localizar así, probablemente hay un fallo de
  accesibilidad: así se encontró que el botón del FilePicker se anunciaba como «Llave».
- Los textos son los reales de `es.ts` (los tests corren en español).
- Popovers, menús y diálogos se renderizan en un portal: búscalos en `page`, no en `screen`.
- Tauri: `mockTauri({ 'plugin:dialog|open': () => '/ruta' })` de `src/tests/tauri.ts`.
- `expect.requireAssertions` está activo: un test sin aserciones falla.

### E2E

- Importar `test` y `expect` de `e2e/support/console.ts`: **cualquier error en la consola
  del navegador hace fallar el test**.
- `waitForApp(page)` antes de interactuar (la SPA no tiene contenido hasta hidratar).
- Tauri simulado: `mockTauri(page, respuestas)`, `tauriCalls(page)` y `emitTauriEvent(page, …)`
  de `e2e/support/tauri.ts` (p. ej. el arrastrar y soltar nativo del FilePicker).
- Los tests de la app corren contra el **build de producción** (`vite preview`); los del
  Workbench, contra el servidor de desarrollo.

### Visuales

Una captura por historia y tema, comparada **píxel a píxel** (tolerancia cero): un cambio de
2 px en un radio se detecta. Las capturas de referencia (`e2e/__screenshots__`) **se
versionan**: son el resultado esperado del test, no un artefacto. Ignorarlas haría que cada
máquina generara su propia referencia y el test no detectaría nunca una regresión.

**Determinismo.** La captura es idéntica en cualquier máquina Linux. Cada historia se captura
**aislada** (`?story=`): su captura no depende de las historias que tenga encima.

| Fuente de variación                  | Cómo se neutraliza                                                                                                                                                                                                     |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Fuentes del sistema (`system-ui`)    | Inter y JetBrains Mono versionadas en `e2e/fixtures/fonts` (OFL), verificadas por `SHA256SUMS` en cada ejecución. Si un carácter visible no tiene glifo en ellas, el test falla en vez de usar una fuente del sistema. |
| Rasterizado (hinting, subpíxel, LCD) | Flags de Chromium del proyecto `visual` (`playwright.config.ts`).                                                                                                                                                      |
| Versión del navegador                | Versión exacta de `@playwright/test` (fija el build de Chromium).                                                                                                                                                      |
| Node y pnpm                          | `.node-version` y `packageManager` de `package.json`.                                                                                                                                                                  |
| Fecha y hora                         | Reloj congelado (`FROZEN_NOW`).                                                                                                                                                                                        |
| Animaciones, cursor, movimiento      | Desactivados en la captura y con `reducedMotion`.                                                                                                                                                                      |
| Sistema operativo                    | No corregible (Windows y macOS rasterizan el texto con otros motores): el test **falla** fuera de Linux.                                                                                                               |

Verificado: con las fuentes del sistema sustituidas por otras completamente distintas y con
una configuración de rasterizado opuesta (hinting completo, subpíxel RGB), las 96 capturas
coinciden píxel a píxel.

**Entorno canónico: la CI** (`.github/workflows/client.yml`, job `visual`, `ubuntu-24.04`).
El workflow es **solo manual** (Actions → `client` → _Run workflow_), porque es pesado y los
minutos de Actions en repos privados son limitados: se lanza antes de fusionar o publicar, no
en cada commit. El selector `suites` elige qué ejecutar (`todo`, `check`, `e2e` o `visual`).
En el día a día, la validación es local: `pnpm check` y, si se tocó la interfaz,
`pnpm test:e2e` y `pnpm test:visual`.

Para regenerar las referencias tras un cambio visual intencionado:

- En una máquina Linux: `pnpm test:visual:update`, revisar las imágenes en el diff y confirmar.
- Desde cualquier sistema: lanzar el workflow con `suites` = `visual` y `update_snapshots`,
  descargar el artefacto `visual-snapshots` y confirmarlo en `e2e/__screenshots__`.

Si falla en la CI, el artefacto `visual-diff` contiene la captura esperada, la obtenida y la
diferencia.

Las fuentes de test solo existen en los tests: la app sigue usando las del sistema. Para añadir
caracteres fuera de latin/latin-ext (p. ej. emoji), hay que añadir su fuente a
`e2e/fixtures/fonts`, a `SHA256SUMS` y a `e2e/support/visual.ts`.

## 4. Workbench e historias

El Workbench muestra cada componente aislado con todas sus variantes, con el tema y el idioma
conmutables y recarga al guardar. Se abre con `pnpm workbench` en
`/dev/workbench/<componente>`, y `?story=<variante>` aísla una sola.

**Todo componente de `src/lib/components/` tiene su `stories/Nombre.stories.svelte`** (ver §5), con al
menos: variantes, tamaños, estados (error, deshabilitado, carga…) y casos límite (textos
largos, listas largas). Las historias alimentan automáticamente los tests de humo, de
accesibilidad y visuales: añadir una historia es añadir tests.

```svelte
<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = { title: 'Badge', group: 'Estado', description: 'Estado corto.' };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import Badge from './Badge.svelte';
</script>

<Story name="Variantes">
    <Badge>neutral</Badge>
    <Badge variant="success" dot>activo</Badge>
</Story>

<Story name="Formulario" layout="grid">…</Story>
```

- `layout`: `padded` (fila, por defecto), `grid` (comparar campos), `stack` (columna),
  `centered` (pieza suelta).
- Grupos del menú: Acciones, Formularios, Selección, Superposiciones, Estado, Íconos
  (`GROUP_ORDER` en `src/lib/workbench/stories.ts`).
- Las historias, el Workbench y las rutas `(dev)` son herramientas de desarrollo: pueden tener
  texto literal (están excluidos de `check:i18n`), pero **sí** deben cumplir las reglas de
  estilo. No llegan al build de producción.

## 5. Organización de archivos

La raíz de una carpeta de componentes contiene **solo los componentes**. Historias y tests no se
intercalan con ellos:

```
src/lib/components/ui/
  Button.svelte
  Input.svelte
  …
  stories/          Siempre: todas las historias de la carpeta
    Button.stories.svelte
    Input.stories.svelte
  tests/            Cuando la carpeta tiene más de 5 archivos de test
    Input.svelte.test.ts
    Select.svelte.test.ts
```

- **Historias**: siempre en una carpeta `stories/` junto a la carpeta de sus componentes, en
  cualquier parte de `src/`. Se nombran como el componente: `Input.svelte` →
  `stories/Input.stories.svelte`.
- **Tests**: si una carpeta tiene **más de 5** archivos de test, van todos a una carpeta `tests/`
  (`Input.svelte` → `tests/Input.svelte.test.ts`). Con 5 o menos pueden ir junto al código que
  prueban (`lib/http/client.ts` → `lib/http/client.test.ts`). Al superar el límite, se mueven
  todos, no solo el sexto.
- Los imports desde `stories/` y `tests/` suben un nivel: `import Input from '../Input.svelte';`.
- `pnpm check:structure` (dentro de `pnpm check`) hace cumplir ambas reglas.
