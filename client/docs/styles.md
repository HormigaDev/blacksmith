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
  base.css          Reset y estilos de elementos HTML. Sin clases.
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
  dar estilo a contenido que no controla el componente (p. ej. HTML renderizado), y con un
  comentario que lo justifique.
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

## 5. Estructura del proyecto

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

## 6. Antes de dar un cambio por terminado

```sh
cd client
pnpm format
pnpm check   # svelte-check + check:styles + prettier
```
