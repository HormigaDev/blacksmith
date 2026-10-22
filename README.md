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

## ⚠️ Permisos del agente (obligatorio en cada servidor)

> **Sin este paso, el agente arranca y responde, pero no puede iniciar, parar ni reiniciar
> ningún servicio**: esas acciones fallan con `SYSTEMD_PERMISSION_DENIED`. Ver el estado y
> las métricas sí funciona, así que el problema puede pasar desapercibido hasta el primer
> despliegue.

El agente corre como el usuario sin privilegios `blacksmith` y con `NoNewPrivileges=yes`, de
modo que no puede convertirse en root de ninguna forma. Los permisos para gestionar servicios
se los concede **polkit**, con una regla que solo le permite:

- `start`, `stop` y `restart`,
- de unidades `bbel-<nombre>.service` (el prefijo reservado a los servicios del agente).

Cualquier otra cosa que pida (otras unidades como `ssh` o `nginx`, `enable`, `mask`,
reiniciar el equipo…) se le deniega. Es una lista de permitidos: no hay que mantener una lista
de unidades críticas.

**Instalación** (como root, una vez por servidor; polkit la aplica al momento):

```sh
install -o root -g root -m 0644 deploy/polkit/10-blacksmith.rules /etc/polkit-1/rules.d/
```

**Verificación** (como root). Crea unidades temporales de prueba, comprueba lo permitido y lo
denegado, y lo deja todo como estaba:

```sh
sh deploy/polkit/verify.sh   # debe terminar con «regla correcta»
```

**El agente lo comprueba solo al arrancar**: pide a systemd parar una unidad reservada que no
existe (sin ningún efecto) y registra el resultado. Si falta la regla, verás en
`journalctl -u blacksmith`:

```text
polkit NO autoriza al agente: iniciar, parar o reiniciar servicios fallará con SYSTEMD_PERMISSION_DENIED…
```

y, si está bien, `polkit autoriza al agente a gestionar sus servicios (bbel-*.service)`.

**No use sudoers** (`/etc/sudoers.d/…`) para esto:

- `NOPASSWD: /bin/systemctl` equivale a dar root al agente: `systemctl link` o `systemctl edit`
  bastan para ejecutar cualquier cosa como root.
- Los comodines de sudoers coinciden con espacios: `systemctl start bbel-*` también permite
  `systemctl start bbel-x ssh.service`.
- Además, no funciona: `NoNewPrivileges=yes` impide que `sudo` eleve privilegios, y quitarlo
  obligaría a desmontar buena parte del endurecimiento de `deploy/blacksmith.service`.

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
- `deploy/polkit/10-blacksmith.rules` y `verify.sh`: permisos del agente sobre sus servicios
  (obligatorio: ver la sección «Permisos del agente» más arriba).

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
