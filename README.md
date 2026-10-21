# Blacksmith

Monorepo Rust con tres componentes:

- `client/`: aplicación de escritorio Tauri 2 con SvelteKit.
- `agent/`: servidor HTTP del agente, construido con Axum.
- `shared/`: tipos serializables que comparten el cliente y el agente.

## Requisitos

- Rust 1.97.1, fijado en `rust-toolchain.toml` (rustup lo instala solo). `agent` y `shared`
  usan la edición 2024.
- Node.js y pnpm para el frontend.
- Dependencias del sistema de Tauri 2 para la plataforma elegida. En Linux se
  requieren GTK 3, WebKitGTK 4.1 y sus encabezados de desarrollo; consulta la
  [guía oficial de prerrequisitos de Tauri](https://v2.tauri.app/start/prerequisites/).

## Desarrollo

Desde la raíz del repositorio:

```sh
cp .env.template .env
cargo check --workspace
cargo run -p agent
cargo clippy -p agent -p shared --all-targets -- -D warnings
cargo test -p agent -p shared
```

El agente escucha en `127.0.0.1:4040` y publica `GET /api/v1/health`. Las rutas de
administración solo aceptan peticiones con `Host` loopback, sin `Origin`/`Sec-Fetch-Site` y sin
cabeceras de proxy:

```sh
curl http://127.0.0.1:4040/api/v1/health
```

La configuración (`BLACKSMITH_*`) está documentada en `.env.template`. Se valida completa al
arrancar: si una variable es desconocida o tiene un valor fuera de rango, el agente no arranca.
Solo los builds de desarrollo leen `.env`.

### Despliegue

- `deploy/blacksmith.service`: unit de systemd endurecida (`Type=notify`, usuario sin
  privilegios, solo red loopback, secretos vía `LoadCredential=`).
- `deploy/nginx.conf`: proxy TLS que solo expone `POST /api/webhooks/github`.

```sh
cargo build --release -p agent   # target/release/blacksmith-agent
```

Para el cliente de escritorio:

```sh
cd client
pnpm install
pnpm tauri dev
```

El frontend también se puede ejecutar en el navegador con `pnpm dev`.
