# Componentes, HTTP y dependencias del cliente

Lectura obligatoria antes de crear una página o un componente. Complementa a
[styles.md](styles.md) (tokens y estilos) y a [i18n.md](i18n.md) (textos).

## 1. Regla principal: reutilizar antes de crear

Toda pantalla se construye con los componentes de `src/lib/components/ui/`. **Nunca** se usa un
`<input>`, `<select>`, `<textarea>`, `<button>` o `<input type="checkbox">` nativo en una página:
su aspecto, su accesibilidad y su comportamiento ya están resueltos aquí. Si falta una variante,
se añade al componente existente; si falta un componente, se crea en `ui/` siguiendo este
documento.

Para verlos aislados, con todas sus variantes: `pnpm workbench` (ventana de Tauri) o
`pnpm workbench:web` (navegador). Ver [testing.md](testing.md) §4.

```svelte
<script lang="ts">
    import { Button, Input, Select } from '$lib/components/ui';
</script>
```

## 2. Catálogo

| Componente          | Uso                                                                                                                                                                                                                                                                                                                           |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Button`            | `variant` primary · secondary · ghost · danger; `size`; `loading`; `iconOnly` (con `aria-label`). `type="button"` por defecto.                                                                                                                                                                                                |
| `Input`             | **Único input de texto.** `type` text · email · password · search · url · tel · number. Snippets `leading` / `trailing` para íconos. `password` incluye mostrar/ocultar. `number` oculta las flechas nativas y muestra − / + flotando por encima del borde al enfocar (mantener pulsado repite; respetan `min`/`max`/`step`). |
| `Textarea`          | Crece con el contenido hasta `maxRows`. Snippet `trailing`. Con `maxlength` muestra un contador.                                                                                                                                                                                                                              |
| `Select`            | Lista accesible (teclado, typeahead). `options: { value, label, disabled? }[]`. Snippet `leading`. `hideLabel` para barras de herramientas.                                                                                                                                                                                   |
| `DatePicker`        | Campo segmentado + calendario. Valor ISO `AAAA-MM-DD`; `min` / `max`.                                                                                                                                                                                                                                                         |
| `TimePicker`        | Campo segmentado. Valor ISO `HH:mm` (o `HH:mm:ss` con `granularity="second"`); 12/24 h según el idioma o `hourCycle`.                                                                                                                                                                                                         |
| `FilePicker`        | Devuelve **rutas** absolutas (diálogo nativo y arrastrar y soltar de Tauri). `accept` = extensiones sin punto; `multiple`; `directory`.                                                                                                                                                                                       |
| `Checkbox`          | Con `description` e `indeterminate`.                                                                                                                                                                                                                                                                                          |
| `Switch`            | Ajustes que se aplican al momento.                                                                                                                                                                                                                                                                                            |
| `ToggleGroup`       | Selección única segmentada (vista, rango…). Opciones con `icon`; `iconOnly`.                                                                                                                                                                                                                                                  |
| `ToggleButton`      | Botón con estado activado/desactivado (`pressed`).                                                                                                                                                                                                                                                                            |
| `Dialog`            | Modal genérico con `title`, `description`, contenido y snippet `footer`.                                                                                                                                                                                                                                                      |
| `ConfirmDialog`     | **Obligatorio antes de cualquier acción destructiva** (parar un servicio, borrar variables, desplegar). `variant="danger"`; `onConfirm` asíncrono con estado de carga; si lanza, sigue abierto.                                                                                                                               |
| `Tooltip`           | Ayuda contextual. El disparador recibe las props por el snippet: `{#snippet children(props)}<Button {...props}>`.                                                                                                                                                                                                             |
| `Badge`             | Estado corto: neutral · accent · success · warning · danger · info; `dot`.                                                                                                                                                                                                                                                    |
| `Spinner`           | Indicador de carga; con `label` se anuncia a lectores de pantalla.                                                                                                                                                                                                                                                            |
| `Field`             | Base interna de los controles: etiqueta, ayuda, error y ARIA. Úsese al crear un control nuevo.                                                                                                                                                                                                                                |
| `toast` + `Toaster` | `toast.success / error / warning / info(t('…'), { description })`. `Toaster` ya está en el layout raíz.                                                                                                                                                                                                                       |

Convenciones comunes de los controles de formulario:

- Siempre `label` (obligatorio, también por accesibilidad); `hint` para ayuda; `error` para
  validación (pinta el borde y enlaza el mensaje por `aria-describedby`); `required`; `disabled`.
- Valores con `bind:value` / `bind:checked` / `bind:pressed`, o bien `onValueChange`.
- Fechas y horas viajan como texto ISO, nunca como tipos de la librería: así se puede cambiar
  de librería sin tocar las pantallas.

## 3. Cómo se construye un componente

- Svelte 5 con runes y snippets (ver [styles.md](styles.md) §4).
- Los primitivos accesibles (foco atrapado, teclado, ARIA, posicionamiento de popovers) vienen de
  **bits-ui**, que es _headless_: no trae estilos, los ponemos nosotros con tokens. No se
  reimplementa a mano lo que bits-ui ya resuelve.
- Estilos de los elementos que renderiza bits-ui: el `<style>` con alcance no les llega (y menos
  dentro de un portal). Por eso:
    - Lo que comparten varios componentes vive en `src/lib/styles/controls.css`, con clases `bs-*`
      (`.bs-control`, `.bs-popover`, `.bs-option`, `.bs-segment`, `.bs-toggle`, `.bs-dialog`…).
    - Lo exclusivo de un componente va en su `<style>` como `:global(.bs-<nombre>)`, con un
      comentario que lo justifique. Las clases `bs-*` solo se usan dentro de `components/ui/`.
- Íconos, dos sets con papeles distintos:
    - **Lucide** (`@lucide/svelte`), set por defecto de la interfaz (controles, acciones,
      navegación), por su trazo uniforme: `import ServerIcon from '@lucide/svelte/icons/server';`.
    - **Font Awesome Free** (`@fortawesome/free-{solid,regular,brands}-svg-icons`) con
      `<FaIcon icon={faGithub} />`: sobre todo **marcas** (GitHub, distribuciones Linux, runtimes),
      que Lucide no tiene, y glifos rellenos puntuales. No se mezclan ambos sets en un mismo grupo
      de controles. `FaIcon` dibuja el SVG sin `fontawesome-svg-core` (sin CSS inyectado) y solo se
      empaquetan los íconos importados.
    - Su tamaño y color salen de los tokens (`--icon-size-*`, `currentColor`); nunca se les pasa
      un tamaño en píxeles ni un color.
- Todo texto visible sale de `t()`, también los `aria-label` internos.

## 4. Cliente HTTP (`$lib/http`)

Vanilla sobre la API de Fetch: **no se usa Axios** ni ninguna librería HTTP.

```ts
import { createHttpClient, describeError, HttpError } from '$lib/http';

const agent = createHttpClient({ baseUrl: 'http://127.0.0.1:4040/api/v1/', fetch: agentFetch });

try {
    const status = await agent.get('health', { parse: parseAgentStatus });
} catch (error) {
    toast.error(describeError(error)); // traduce por el `code` del agente
}
```

- **Transporte inyectable.** La webview **no** puede llamar al agente directamente: la CSP solo
  permite IPC, y el agente rechaza toda petición con cabecera `Origin` (protección contra CSRF y
  DNS rebinding del túnel SSH). Para el agente se pasa un `fetch` que se ejecuta en Rust, con la
  misma firma que el estándar (p. ej. el de `@tauri-apps/plugin-http`). **No se relaja la CSP ni
  la guarda del agente para que funcione el `fetch` del navegador.**
- **Errores tipados**, todos heredan de `RequestError`: `HttpError` (`status`, `code` estable de
  `shared::ErrorCode`, `detail` solo para registro), `NetworkError`, `TimeoutError`,
  `AbortedError`, `ResponseFormatError`. Al usuario se le muestra `describeError(error)`.
- **Reintentos** con backoff exponencial y jitter, respetando `Retry-After`, **solo** en métodos
  idempotentes (GET, HEAD, PUT, DELETE) y ante red caída, tiempo agotado, 408, 429, 502, 503 o 504.
  Un POST nunca se repite: podría duplicar una acción.
- `timeoutMs` por petición (15 s por defecto) y `signal` para cancelar.
- Cada petición lleva `x-request-id` (el mismo en todos sus reintentos), que el agente registra.
- Seguridad: `path` siempre relativo a `baseUrl` (rechaza URLs absolutas, `//host` y `../`);
  `redirect: 'error'`.
- `parse` valida la respuesta: lo que llega de fuera es una frontera de confianza.
- `API_ERROR_CODES` debe reflejar `shared/src/error.rs`. Al añadir un código en Rust, se añade
  aquí y en `errors.agent.<CODE>` de `es.ts` (si falta la traducción, no compila).

## 5. Tests

Tests unitarios, de componente, E2E, de accesibilidad y visuales, y el Workbench: ver
[testing.md](testing.md). Todo componente nuevo lleva sus tests y sus historias (`stories/`),
organizados según [testing.md](testing.md) §5.

## 6. Dependencias (cadena de suministro)

`pnpm-workspace.yaml` impone:

- `minimumReleaseAge: 10080`: **ninguna versión con menos de 7 días** desde su publicación. La
  mayoría de los paquetes comprometidos se detectan y retiran antes.
- `saveExact: true`: versiones exactas; actualizar es un cambio deliberado y revisado.
- Scripts de instalación bloqueados salvo la lista `onlyBuiltDependencies`.

Antes de añadir una dependencia: ¿lo resuelve la plataforma (Fetch, `Intl`, `node:test`)? ¿Lo
resuelve una ya instalada? Si de verdad hace falta, debe tener mantenimiento activo y pocas
dependencias propias. **No se añaden excepciones a `minimumReleaseAgeExclude`** salvo para
corregir una vulnerabilidad, y dejándolo documentado.

`@tauri-apps/api` y los plugins JS deben coincidir en major.minor con sus crates de Rust
(`src-tauri/Cargo.toml`).
