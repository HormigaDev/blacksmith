# Conexión SSH, túnel y credenciales

Lectura obligatoria antes de tocar `client/src-tauri/src/{ssh,secrets.rs,commands.rs}` o
`client/src/lib/ssh/`. Es la pieza más sensible del cliente: maneja llaves privadas.

## 1. Qué hace

El cliente abre un **túnel SSH** contra el VPS con la llave del usuario y reenvía un puerto local
aleatorio de `127.0.0.1` al `127.0.0.1:4040` del VPS, donde escucha el agente (equivalente a
`ssh -N -L <local>:127.0.0.1:4040 usuario@vps`). No hay login propio: la autenticación es la de
OpenSSH.

Biblioteca: **russh** (Rust puro, asíncrona sobre tokio, sin C). Se descartó `ssh2` (libssh2 +
OpenSSL, bloqueante). Backend criptográfico `ring`, sin `aws-lc-rs` (que exige compilar C con cmake).

## 2. Garantías de seguridad

| Riesgo                                     | Medida                                                                                                                                                                                                                                                       |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Intermediario (MITM)                       | Verificación de la llave del servidor: `~/.ssh/known_hosts` y registro propio (`known_hosts.json`). La primera vez el usuario confirma la huella (TOFU, como OpenSSH). Una llave distinta a la registrada —incluido otro algoritmo— **bloquea** la conexión. |
| La webview fija una llave arbitraria       | `tunnel_trust_host` solo acepta la huella que Rust acaba de presentar para ese `host:puerto`.                                                                                                                                                                |
| Llave privada en disco en claro            | Cifrado por envoltura: el llavero del sistema guarda solo una clave maestra aleatoria de 32 bytes; cada credencial se cifra con AES-256-GCM en el directorio de datos de la app (`0600`).                                                                    |
| Archivo de credencial cambiado de servidor | El id de la credencial va como dato autenticado de AES-GCM: no descifra en otro servidor.                                                                                                                                                                    |
| Límite del llavero de Windows (2560 B)     | Por el cifrado por envoltura, el llavero nunca guarda la llave (una RSA de 4096 bits no cabría).                                                                                                                                                             |
| Llave expuesta a la webview                | La llave cruza a Rust una sola vez (al guardarla) y nunca vuelve: los comandos devuelven algoritmo y huella. La interfaz borra la llave pegada en cuanto Rust la guarda.                                                                                     |
| Secretos en memoria y en registros         | `Zeroizing` para llave y passphrase; `Debug` de las credenciales muestra `[REDACTED]`; los errores no llevan material secreto.                                                                                                                               |
| Passphrase                                 | Solo se guarda si el usuario marca «recordar». Si no, se pide al conectar.                                                                                                                                                                                   |
| Entradas maliciosas desde la webview       | `SshTarget` valida host (nombre DNS o IP), usuario (POSIX, nunca empieza por `-`) y puerto; las llaves se leen con un límite de 64 KiB.                                                                                                                      |
| Comandos expuestos                         | Los comandos están declarados en `build.rs` (app manifest): cada uno necesita un permiso explícito en `capabilities/default.json`.                                                                                                                        |
| Exposición del túnel                       | El puerto local escucha solo en `127.0.0.1`. Lo puede usar cualquier proceso local del usuario (igual que `ssh -L`); el agente sigue aplicando su guarda (sin `Origin`, `Host` loopback).                                                                    |

## 3. Errores

Cada fallo tiene un `code` estable (`CommandError::code()` en `error.rs`), reflejado en
`TUNNEL_ERROR_CODES` (`src/lib/ssh/errors.ts`) y traducido en `errors.tunnel.<CODE>` de `es.ts`.
Se distinguen, entre otros: llave no encontrada, llave pública en lugar de privada, passphrase
requerida o incorrecta, llavero no disponible, servidor inalcanzable, tiempo agotado, llave de
servidor desconocida o cambiada, llave rechazada, **agente caído con SSH funcionando** y puerto
local ocupado. Al añadir un código: los tres sitios a la vez (si falta la traducción, no compila).

## 4. Comandos (`src/lib/ssh/api.ts`)

| Función            | Comando de Rust          | Qué hace                                                                                                |
| ------------------ | ------------------------ | ------------------------------------------------------------------------------------------------------- |
| `saveKey`          | `ssh_save_key`           | Lee (archivo, contenido o `~/.ssh`), valida, guarda cifrada y **registra el servidor**.                 |
| `listServers`      | `servers_list`           | Servidores configurados (`servers.json`, sin secretos).                                                 |
| `removeServer`     | `server_remove`          | Por id. Si es SSH, cierra su túnel y borra su llave. Confirmar antes. La huella confirmada se conserva. |
| `addLocalServer`   | `server_add_local`       | Registra este equipo (sin túnel ni llave).                                                              |
| `localAgentStatus` | `local_agent_status`     | Comprueba el agente en `127.0.0.1:<puerto>`.                                                            |
| `openTunnel`       | `tunnel_open`            | Abre el túnel (idempotente).                                                                            |
| `trustHostKey`     | `tunnel_trust_host`      | Confirma la huella recién presentada.                                                                   |
| `forgetHostKey`    | `ssh_forget_host`        | Olvida la llave registrada del servidor. Confirmar antes.                                               |
| `closeTunnel`      | `tunnel_close`           | Cierra el túnel y libera el puerto.                                                                     |
| `onTunnelClosed`   | evento `tunnel://closed` | Cierre pedido o pérdida de conexión (keepalive cada 15 s, ~45 s).                                       |

## 5. Servidores guardados

Un servidor se registra en cuanto su llave es válida (`ssh_save_key`), **antes** de intentar
conectar: si la conexión falla, no hay que volver a introducir nada. Si SSH funciona pero el
agente no responde (`AGENT_UNREACHABLE`), el túnel se cierra y el servidor queda listado como
«agente no configurado», listo para reintentar.

En la interfaz, `servers` (`src/lib/servers/servers.svelte.ts`) es la única fuente de verdad del
estado de cada servidor (`disconnected`, `connecting`, `connected`, `agentMissing`, `error`), de la
huella pendiente de confirmar y de la passphrase pendiente. `AddServerForm` solo da de alta;
`ServerCard` muestra el estado y las acciones; `ServerList` los dispone en lista o cuadrícula
(preferencia local, `servers.view`) y permite reordenarlos. El orden se guarda en `servers.json`
con `servers_reorder`, que solo acepta una permutación exacta de los ids existentes (ni altas ni
bajas por esa vía); si falla, la interfaz avisa y recarga el orden real.

### Servidor local (este equipo)

Si el host es `localhost` o una IP de loopback (127.0.0.0/8, `::1`), el servidor se registra
como **local** (`kind: 'local'`, id `local:<puerto>`): no hay túnel ni llave, y el cliente habla
directamente con el agente en `127.0.0.1:<puerto>` (4040 por defecto). Sirve para probar el
cliente contra un agente levantado en la misma máquina, sin desplegar nada en un VPS:

```sh
cargo run -p agent            # en la raíz: escucha en 127.0.0.1:4040 (ver .env)
cd client && pnpm tauri dev   # Añadir servidor → host «localhost»
```

`local_agent_status` solo consulta `127.0.0.1`: el comando no acepta un host, así que no puede
usarse para sondear otras máquinas.

### Comprobación del agente

«Conectado» significa lo mismo en los dos tipos de servidor: el agente respondió a
`GET /api/v1/health` con un `AgentStatus` cuyo nombre es `shared::AGENT_NAME`
(`src-tauri/src/agent_probe.rs`). En un servidor SSH la comprobación se hace a través del túnel,
ya abierto; si falla, el túnel se cierra. Se distingue:

- `AGENT_UNREACHABLE`: no hay nadie escuchando, o no contesta a tiempo.
- `AGENT_INVALID`: algo contesta en ese puerto, pero no es el agente (otro servicio, otra versión
  del contrato).

La petición es HTTP/1.1 mínimo, sin cliente HTTP como dependencia: lectura incremental con límite
de 64 KiB y timeout, sin esperar a que el otro extremo cierre la conexión (un servicio que no
habla HTTP podría no cerrarla nunca).

## 6. Tests

- **Rust** (`cargo test -p client`): validación de destinos, decodificación de llaves (ed25519,
  RSA OpenSSH y PKCS#1, ECDSA, cifrada), bóveda (cifrado, manipulación, cambio de servidor,
  otra clave maestra), verificación de llaves de servidor y, **de extremo a extremo** contra un
  servidor SSH real en proceso (russh) con un agente de eco: reenvío, RSA con SHA-2, llave
  cambiada, llave no autorizada, servidor inalcanzable frente a agente caído, timeout, puerto
  ocupado y pérdida de conexión.
- Llavero real del sistema (escribe y borra una entrada de test):
  `cargo test -p client -- --ignored llavero_real`.
- **Interfaz**: `AddServerForm`, `ServerCard` y `servers.test.ts` con Tauri simulado (validación, passphrase,
  confirmación de huella, llave cambiada).
- Las llaves de `src-tauri/src/ssh/fixtures/` son **solo de test**: públicas a propósito, nunca se
  usan en un servidor real. Los escáneres de secretos pueden marcarlas.

## 7. Pendiente

- Llaves FIDO (`sk-*`) y `ssh-agent`: no soportados todavía.
