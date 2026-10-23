# Reglas de estilo del cliente

Obligatorias para cualquier página o componente de `client/`. `pnpm check` las verifica
(`pnpm check:styles` en concreto); un cambio que no pase no está terminado.

## 1. Regla principal: nada de valores quemados

Ningún componente ni página escribe un valor de identidad visual. Siempre se usa una variable CSS.

| Qué                                     | Token                                                                               | Nunca                     |
| --------------------------------------- | ----------------------------------------------------------------------------------- | ------------------------- |
| Colores                                 | `--color-*`                                                                         | `#fff`, `rgb()`, `red`    |
| Sombras                                 | `--shadow-sm / md / lg`                                                             | `0 2px 4px …`             |
| Tipografía                              | `--font-sans`, `--font-mono`, `--font-size-*`, `--font-weight-*`, `--line-height-*` | `14px`, `600`, `Arial`    |
| Espaciado (padding, margin, gap, inset) | `--space-*`                                                                         | `12px`, `1rem`            |
| Radios                                  | `--radius-*`                                                                        | `8px`                     |
| Capas                                   | `--z-*`                                                                             | `z-index: 10`             |
| Duraciones y curvas                     | `--duration-*`, `--easing-*`                                                        | `200ms`, `ease-in-out`    |
| Alturas de control                      | `--control-height-*`                                                                | `height: 36px` en botones |
| Bordes                                  | `--border-width`, `--border-width-strong`                                           | `1px` repetido            |

Se permiten `0`, `auto`, `none`, `inherit`, `100%`, `50%`, `transparent` y `currentColor`. Las
medidas de layout estructural (`height: 100vh`, `grid-template-columns: 1fr 2fr`) no son
identidad y se escriben tal cual.

Si falta un token, **se añade el token**; nunca se escribe el valor directamente. Los tokens
generales van en `tokens.css` y los colores y sombras en **todos** los temas.

Con `calc()` se combinan tokens: `margin: calc(-1 * var(--space-2))`.

## 2. Arquitectura

```
src/lib/styles/
  global.css        Punto de entrada. Se importa SOLO en src/routes/+layout.svelte
  tokens.css        Tokens que no dependen del tema (tipografía, espaciado, radios, capas…)
  base.css          Reset, elementos HTML, barra de scroll global y ocultación de las flechas
                    nativas de <input type="number">. Sin clases.
  controls.css      Capa compartida de los componentes de ui/ (clases `bs-*`). Ver components.md
  themes/
    light.css       Tema de REFERENCIA: define el contrato completo de color y sombra
    dark.css        Un archivo por tema, con exactamente las mismas variables
src/lib/theme/
  themes.ts         Registro de temas (id, nombre, esquema claro/oscuro)
  theme.svelte.ts   Estado reactivo: preferencia, tema efectivo y aplicación al DOM y la ventana
```

El tema activo es `data-theme="<id>"` en `<html>`. El script de `src/app.html` lo fija antes del
primer pintado para evitar destellos. Si el id no es válido, se aplica `light.css`.

### Tokens de color semánticos

Los nombres describen el **uso**, no el color. Nunca existe `--color-orange`.

- Fondos: `--color-bg` (ventana), `--color-bg-subtle` (paneles laterales), `--color-surface`
  (tarjetas), `--color-surface-raised` (menús, popovers), `--color-surface-hover`,
  `--color-surface-active`, `--color-overlay` (fondo de modales).
- Bordes: `--color-border`, `--color-border-strong`.
- Texto: `--color-text`, `--color-text-muted` (secundario), `--color-text-subtle` (placeholder,
  ayuda), `--color-text-disabled`, `--color-text-inverse`.
- Acento: `--color-accent`, `-hover`, `-active`, `-subtle` (fondo tenue), `--color-on-accent`
  (texto sobre el acento).
- Estados: `success`, `warning`, `danger` e `info`, cada uno con su color, `-subtle` y `on-`.
  Ejemplo: `--color-danger`, `--color-danger-subtle`, `--color-on-danger`.
- Interacción: `--color-focus-ring`, `--color-selection`, `--color-scrollbar`.

Combinaciones correctas: texto `--color-on-X` sobre fondo `--color-X`; texto `--color-X` sobre
fondo `--color-X-subtle`. Cualquier otra combinación debe comprobarse en todos los temas.

## 3. Añadir un tema

1. Copiar `themes/dark.css` (o `light.css`) a `themes/<id>.css`, con el selector
   `:root[data-theme="<id>"]` y `color-scheme: light | dark`.
2. Añadir `@import "./themes/<id>.css";` en `global.css`.
3. Registrarlo en `THEMES` de `src/lib/theme/themes.ts`.
4. Añadir su nombre visible en `theme.names.<id>` de `src/lib/i18n/messages/es.ts`. Si falta,
   no compila.

`pnpm check:styles` falla si el tema no declara todas las variables de `light.css`, si declara
alguna que no existe en ella, si falta el import o si falta el registro. Para añadir un token de
color nuevo: primero en `light.css`, después en todos los demás temas.

## 4. Reglas de componentes (Svelte 5)

- **Runes siempre**: `$props()`, `$state()`, `$derived()`, `$effect()`. Nada de `export let`,
  `$:` ni stores de `svelte/store` en código nuevo.
- **Snippets, no slots**: `children: Snippet` y `{@render children()}`.
- **Eventos como props** (`onclick`, `onchange`), no `createEventDispatcher` ni `on:click`.
- Props tipadas en la desestructuración: `let { label, variant = "primary" }: Props = $props();`
- **Estilos con alcance** en el `<style>` del componente. Prohibido `:global(...)` salvo para
  dar estilo a contenido que no controla el componente (elementos de bits-ui, HTML renderizado),
  y con un comentario que lo justifique. Las páginas nunca usan las clases `bs-*` de
  `controls.css`: usan los componentes de `$lib/components/ui` ([components.md](components.md)).
- La barra de scroll ya tiene estilo global: no se redefine en componentes.
- Clases con nombres semánticos y cortos (`.header`, `.title`). Sin utilidades tipo Tailwind.
- Variantes por props que se traducen a clases o a variables privadas del componente
  (prefijo `--_`), nunca a colores en línea:

    ```svelte
    <button class="button" class:danger={variant === 'danger'}>…</button>

    <style>
        .button {
            --_bg: var(--color-accent);
            --_fg: var(--color-on-accent);
            background-color: var(--_bg);
            color: var(--_fg);
        }
        .danger {
            --_bg: var(--color-danger);
            --_fg: var(--color-on-danger);
        }
    </style>
    ```

- Nada de `style="..."` con valores de identidad. `style:` solo para valores dinámicos reales
  (p. ej. el ancho de una barra de progreso).
- Accesibilidad: elementos nativos (`button`, `a`, `label`, `select`); foco visible (lo da
  `base.css`; no se quita con `outline: none` sin reemplazo); el texto muted nunca se usa para
  información crítica.
- El movimiento usa `--duration-*`, que valen `0ms` con `prefers-reduced-motion`.

## 5. Marco fijo de la ventana (obligatorio)

Blacksmith es una aplicación de escritorio, no una web: **la ventana nunca se desplaza entera**.
El marco (`AppShell`: barra lateral, barra de título con los controles de ventana) queda siempre
fijo y visible; solo hacen scroll las zonas de contenido. Una página que empuja el marco fuera
de la pantalla es un fallo grave, aunque «funcione».

- El documento no tiene scroll: `html { overflow: hidden }` y `body { overflow: clip }` en
  `base.css`. No se cambia.
- `AppShell` está fijado a la ventana (`position: fixed; inset: 0`) y todas sus pistas de
  rejilla están acotadas (`minmax(0, 1fr)`, nunca una fila implícita `auto`), con
  `min-height: 0` y `overflow: hidden` en sus columnas. WebKitGTK (el motor de Tauri en Linux)
  deja que una fila `auto` crezca con el contenido aunque Chromium no lo haga: **que se vea bien
  en Chromium no prueba nada**.
- Solo hacen scroll contenedores explícitos (`overflow: auto` + `min-height: 0`): el contenido de
  la página (`.content`) y listas largas (la de servidores). Un componente nuevo con scroll
  propio sigue el mismo patrón y usa `overscroll-behavior: contain`.
- Nada debe desbordar sin querer un contenedor con scroll (márgenes negativos, subrayados que se
  salen…): aparece una barra de scroll falsa. Si un elemento necesita salirse, se dibuja dentro
  (sombra interior, `outline-offset` negativo).
- Overlays (diálogos, menús, toasts) van en portal con `position: fixed`; nunca estiran el
  documento.
- Lo verifica el E2E «el marco queda fijo» (`e2e/app/tauri.spec.ts`), que también corre en
  WebKit. Cualquier página nueva alta debe probarse con la ventana baja (p. ej. 1024×480).

### Tamaños de ventana

La interfaz funciona a **cualquier tamaño**, de un móvil (futura PWA) a una pantalla grande.
No se confía en el tamaño mínimo de la ventana: los gestores de ventanas en mosaico de Linux lo
ignoran, y GTK entonces recorta el webview en lugar de redimensionarlo. Por eso el mínimo de
`tauri.conf.json` es bajo (360×520) y el diseño se adapta.

- Un único punto de corte, **compacto: `@media (max-width: 48rem)`**. CSS no admite `var()` en
  las media queries; `check:styles` rechaza cualquier otro valor.
- En compacto, `AppShell` pasa a una columna: la barra lateral es un panel superpuesto que se
  abre con el botón de menú de la barra de título.
- Nada de anchos mínimos fijos que desborden: rejillas con
  `minmax(min(100%, var(--layout-card-min-width)), 1fr)` y anchos con `min(100%, …)`. Las
  cabeceras con acciones usan `flex-wrap`.
- Se prueba con ventana estrecha (≈ 400 px) y baja (≈ 480 px), no solo a 1280×800.

antes de dar una pantalla por terminada, pregúntate si como cliente
aceptarías la app así (marco que se mueve, controles fuera de la vista, barras de scroll
falsas, textos truncados). Si la respuesta es no, no está terminada.

## 6. Estructura del proyecto

```
src/
  app.html                  Documento base y script de tema previo al pintado
  routes/
    +layout.svelte          Raíz: estilos globales, tema y <html lang>
    +layout.ts              ssr = false (SPA para Tauri) y carga del idioma
    (app)/                  Pantallas de la app configurada, dentro de AppShell
      +layout.svelte
      +page.svelte          Dashboard
    (setup)/                (futuro) flujo de configuración a pantalla completa
  lib/
    components/
      layout/               Marcos de pantalla (AppShell…)
      ui/                   Primitivas reutilizables (Button, Input, Dialog…)
      <dominio>/            Componentes de un área: servers/, services/, logs/…
    i18n/                   Internacionalización (ver docs/i18n.md)
    theme/                  Sistema de temas
    styles/                 Tokens, temas y base
    app.ts                  Constantes de producto (APP_NAME…)
```

- Todo texto visible sale de `t()`: las reglas están en [i18n.md](i18n.md).

- Componentes en `PascalCase.svelte`; módulos TS en `kebab-case.ts`; el estado con runes en
  `*.svelte.ts`.
- Importaciones con `$lib/...`, nunca rutas relativas que suban más de un nivel.
- Cada página define su `<title>` con `<svelte:head>`:
  `{t('<sección>.title')} · {APP_NAME}`.
- La webview no hace peticiones de red (la CSP solo permite IPC). Todo acceso al agente pasa por
  comandos de Tauri en Rust.

## 7. Antes de dar un cambio por terminado

```sh
cd client
pnpm format
pnpm check   # svelte-check + check:styles + prettier
```
