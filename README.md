# Blacksmith

Monorepo Rust con tres componentes:

- `client/`: aplicación de escritorio Tauri 2 con SvelteKit.
- `agent/`: servidor HTTP del agente, construido con Axum.
- `shared/`: tipos serializables que comparten el cliente y el agente.

## Requisitos

- Rust y Cargo (edición 2021).
- Node.js y pnpm para el frontend.
- Dependencias del sistema de Tauri 2 para la plataforma elegida. En Linux se
  requieren GTK 3, WebKitGTK 4.1 y sus encabezados de desarrollo; consulta la
  [guía oficial de prerrequisitos de Tauri](https://v2.tauri.app/start/prerequisites/).

## Desarrollo

Desde la raíz del repositorio:

```sh
cargo check --workspace
cargo run -p agent
```

El agente escucha en `127.0.0.1:3000` y publica `GET /health`.

Para el cliente de escritorio:

```sh
cd client
pnpm install
pnpm tauri dev
```

El frontend también se puede ejecutar en el navegador con `pnpm dev`.
