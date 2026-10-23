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

- `start`, `stop` y `restart` de `bbel@<servicio>.service` (los servicios),
- `start` y `stop` de `bbel-deploy@<servicio>.service` (sus despliegues),
- `start` de `bbel-nginx.service` y de `bbel-certbot@<dominio>.service` (aplicar la
  configuración de nginx y pedir certificados; ver «Sitios web con nginx»).

Cualquier otra cosa que pida (otras unidades como `ssh` o el propio `nginx`, `enable`, `mask`,
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

## Servicios: plantillas de systemd (obligatorio en cada servidor)

El agente **nunca ejecuta el código de los servicios**. Lo hacen dos plantillas de systemd, como
el usuario sin privilegios `bbel`:

- `bbel-deploy@<servicio>.service`: clona la rama por HTTPS, ejecuta los comandos de
  preparación y activa la versión de forma atómica. Si algo falla, la versión anterior sigue en
  marcha.
- `bbel@<servicio>.service`: ejecuta la versión activa con su comando de arranque.

**Sandbox.** Los comandos no pueden salir del directorio del servicio, y no porque se analicen
(un `cd ..` o una ruta absoluta dentro de un script lo burlarían): lo impide el kernel. Cada
unidad solo puede escribir en `/var/lib/bbel/<servicio>`. El resto del sistema es de solo
lectura y `/home` no existe. Quedan invisibles los directorios de los demás servicios, las
variables (`/etc/bbel`), los datos del agente (`/var/lib/blacksmith`, donde vive el token de
GitHub) y los procesos ajenos. Las herramientas (`node`, `cargo`, `python`…) deben estar
instaladas en el sistema; sus cachés (`$HOME`) quedan dentro del directorio del servicio.

**Versiones y rollback.** Cada despliegue construye una versión nueva junto a las anteriores y
solo la activa si todo fue bien; si falla, la activa sigue en marcha. El agente guarda cuál es la
activa y cuál la anterior, y el runner nunca borra la anterior al podar (se conservan las 3
últimas y esa). **Volver a la anterior** (pestaña Despliegue) la reactiva sin descargar ni
compilar nada y reinicia el servicio; repetirlo vuelve a la otra. Si una versión nueva queda
activa pero el servicio no arranca, el rollback está disponible igualmente.

**Sitios estáticos.** Con una *carpeta pública* (p. ej. `dist`), tras la preparación el runner la
copia a `public/<versión>` con permisos de lectura (`0755`/`0644`, sin enlaces que salgan del
repositorio) y `public/current` apunta a la de la versión activa (también en un rollback). El
directorio del servicio es `0711` y las unidades usan `UMask=0077`: nginx solo puede leer
`public/`, nunca el código, las cachés ni los datos del servicio. Un sitio solo estático no
necesita comando de arranque: no hay proceso, lo sirve nginx.

**Arranque automático.** El agente recuerda qué servicios deben estar en marcha (los que se
iniciaron o desplegaron, y no se detuvieron a mano) y los arranca al iniciarse, p. ej. tras
reiniciar el servidor.

Cada servicio vive en `/var/lib/bbel/<servicio>` (`releases/` + el enlace `app` a la activa; se
conservan las 3 últimas). Variables: `/etc/bbel/env/global.env` (comunes) y
`/etc/bbel/env/services/<servicio>.env` (ganan las del servicio); se aplican al reiniciar. La
especificación de cada despliegue queda en `/var/lib/blacksmith/deployments/<servicio>` (`0600`)
y llega a la unidad como credencial, junto con una copia del token que se borra al terminar.

Instalación (como root, una vez por servidor; requiere `git`):

```sh
useradd --system --home-dir /var/lib/bbel --no-create-home --shell /usr/sbin/nologin bbel
install -m 0644 "deploy/systemd/bbel@.service" "deploy/systemd/bbel-deploy@.service" /etc/systemd/system/
systemctl daemon-reload
sh deploy/systemd/verify-sandbox.sh   # comprueba el sandbox con el systemd real
```

Las plantillas se generan desde el código (`blacksmith-agent write-units <dir>`); un test
comprueba que las de `deploy/systemd/` están al día.

**Modo local** (para probar sin VPS): con `BLACKSMITH_SYSTEMD_SCOPE=user` (por defecto en
desarrollo) los servicios se despliegan en el systemd de tu usuario, sin root: el agente instala
las plantillas en `~/.config/systemd/user` al arrancar y todo vive en `~/.local/state` y
`~/.config/bbel`. Los servicios corren como tu usuario y **sin el sandbox del servidor**: el
systemd de usuario no puede aplicarlo sin privilegios. Úsalo con repositorios en los que confíes.

## Conectar GitHub

El servidor descarga y despliega el código por su cuenta: el portátil solo da las órdenes y se
puede apagar. Por defecto no hay llaves de despliegue ni pasos en GitHub por servicio.

- **Repositorio público:** se clona por HTTPS sin credenciales.
- **Repositorio privado:** al guardar el despliegue o pulsar **Desplegar**, el cliente deja en
  el servidor el token de su GitHub App, por el túnel SSH y sin pasar por la interfaz. El
  servidor lo guarda en `/var/lib/blacksmith/github.json` (`0600`, ilegible para los servicios)
  y `git` lo recibe en cada despliegue como credencial de systemd: nunca en la URL, en los
  argumentos ni en el entorno. El token solo puede **leer contenido** de los repositorios donde
  instalaste la App. Para retirarlo: desconecta GitHub o revoca la App en GitHub.

La autorización del cliente es con *Device Flow*: no hay que pegar tokens ni guardar secretos en
el cliente (el *Client ID* es público). El token queda cifrado en la bóveda de credenciales del
equipo (clave maestra en el llavero del sistema).

### Con la App del proyecto (por defecto)

1. En Blacksmith: **GitHub** (barra lateral) → **Conectar con GitHub** → confirma en el navegador
   el código que muestra la aplicación.
2. **Dar acceso a más repositorios** abre la instalación de la App: elige los repositorios que
   quieras desplegar.

### Con tu propia App (opcional)

1. En GitHub: **Settings → Developer settings → GitHub Apps → New GitHub App**.
2. **GitHub App name**: único en GitHub. **Homepage URL**: cualquiera.
3. **Identifying and authorizing users**: *Callback URL* vacío; **desmarca** *Expire user
   authorization tokens* (el servidor usa el token sin poder renovarlo); **marca** *Enable
   Device Flow*.
4. **Webhook**: desmarca *Active*.
5. **Repository permissions**: solo *Contents* → **Read-only** (*Metadata* → Read-only se añade
   solo). Ningún otro permiso.
6. **Where can this GitHub App be installed?**: *Only on this account* → **Create GitHub App**.
7. Anota el **Client ID** (empieza por `Iv`) y el **slug** (la última parte de
   `https://github.com/apps/<slug>`). No hace falta *client secret* ni *private key*.
8. En Blacksmith: **GitHub** → **Usar mi propia GitHub App** → pega ambos → **Guardar** →
   **Conectar con GitHub**.

### Llave de despliegue (opcional)

Por defecto el servidor descarga con la cuenta de GitHub conectada (solo lectura). Si prefieres
que un servicio no dependa de ninguna cuenta, elige **Acceso → Llave de despliegue** en su pestaña
Despliegue: el servidor genera una llave ed25519 propia (la privada nunca sale de él) que solo
puede **leer ese repositorio**, y la clona por SSH con las llaves de host de github.com fijadas.

La llave hay que añadirla al repositorio una vez (*Settings → Deploy keys*, sin *Allow write
access*). Blacksmith te enseña los pasos y abre la página.

**Registro en un clic (solo con tu propia App).** La API de GitHub solo deja añadir deploy keys
con el permiso *Administration: Read and write*, que también permite crear y **borrar
repositorios**, cambiar su configuración y sus colaboradores. Por eso la App de BBEL **nunca lo
pide**: si algún día se viera comprometida, tus repositorios no quedarían a merced del atacante;
solo podría leer los que autorizaste. Si quieres el registro en un clic, hazlo con tu propia App
(apartado anterior) y añádele *Administration: Read and write*; tras guardar, acepta el permiso
nuevo en *Settings → Applications → Installed GitHub Apps → tu App → Review request*. Así ese
permiso solo lo tiene una App que controlas tú.

### Publicar la App del proyecto (mantenedores)

La App por defecto (`DEFAULT_APP` en `client/src-tauri/src/github/mod.rs`) se configura igual
que en el apartado anterior, con *Where can this GitHub App be installed?* → **Any account**
(Advanced → *Make public* si ya existe). Solo *Contents: Read-only*.

Para revocar la autorización: GitHub → *Settings → Applications → Authorized GitHub Apps*.

## Sitios web con nginx

Ningún servicio se expone directamente: nginx es la única puerta, con TLS, cabeceras de
seguridad y límites. En la pestaña **Web** de cada servicio eliges el tipo de proyecto, los
dominios, el puerto del servicio y si va por HTTPS; el agente genera la configuración, la prueba
con `nginx -t` y la aplica. Si nginx la rechaza, se queda la anterior.

| Tipo | Para | Qué hace |
|---|---|---|
| Sitio estático | HTML/CSS/JS | Sirve la carpeta pública; recursos con caché, archivos ocultos bloqueados. Sin proceso. |
| SPA | React, Vue, Svelte, Angular… | Como el anterior, y las rutas del cliente devuelven `index.html` (sin caché). |
| Aplicación web | SSR, Django, Rails, Express… | Proxy al servicio con WebSocket transparente. |
| API | REST, GraphQL | Proxy con límite de peticiones por IP, cuerpos de 10 MB y tiempos cortos. |
| WebSocket o eventos | Socket.IO, SSE, juegos | Conexiones de hasta una hora, sin búfer. |
| Frontend + API | SPA + backend | La carpeta pública como SPA; `/api/` y `/ws` al servicio. |

**HTTPS.** El sitio se publica primero por HTTP (con el desafío de Let's Encrypt) y el
certificado se pide en segundo plano con certbot (*webroot*: nginx no se detiene). Con él, el
sitio pasa a HTTPS (TLS 1.2/1.3, perfil *intermediate* de Mozilla), con redirección desde HTTP y
HSTS. Si falla (DNS que aún no apunta aquí, puertos 80/443 cerrados), sigue por HTTP, se ve el
motivo y se puede reintentar. La renovación es la del temporizador del paquete certbot, que
recarga nginx al renovar.

**Seguridad.**
- El agente solo genera configuración a partir de valores validados (dominios, un puerto y rutas
  suyas): la API nunca acepta texto de configuración de nginx.
- Lo único que corre como root por cuenta del agente son dos unidades fijas: `bbel-nginx.service`
  (`nginx -t` y recarga) y `bbel-certbot@<dominio>.service` (pedir un certificado). polkit solo
  le deja `start` de esas dos; nunca parar o tocar nginx ni ninguna otra unidad.
- Un dominio pertenece a un solo servicio. Una petición a un host desconocido se cierra sin
  respuesta (y por HTTPS, sin presentar certificado).
- Cabeceras: `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`,
  `Permissions-Policy`, `Cross-Origin-Opener-Policy` y HSTS (solo por HTTPS). El proxy nunca
  reenvía las `X-Forwarded-*` que mande el cliente.
- Un test genera todos los tipos, con y sin HTTPS, y los valida con un nginx real.

**Plantillas editables.** Están en `/etc/bbel/nginx/templates/<tipo>.conf` y describen el
cuerpo del `server` (sus `location`). El envoltorio (puertos, HTTPS, desafío ACME, cabeceras y
logs en `/var/log/nginx/bbel-<servicio>.*.log`) lo genera el agente, así que editar una
plantilla no puede romper el TLS. Variables: `{{root}}`, `{{upstream}}`, `{{service}}`,
`{{server_names}}`, `{{primary}}`. Los snippets comunes están en `/etc/nginx/snippets/bbel/`.
Tras editar una plantilla, vuelve a guardar el sitio desde la pestaña Web para regenerarlo.

Instalación (como root, desde la raíz del repositorio; requiere `nginx` y, para HTTPS,
`certbot`):

```sh
apt install nginx certbot acl
sh deploy/nginx/install.sh            # servidor
sh deploy/nginx/install.sh --local    # modo local (con privilegios de root, desde tu usuario)
sh deploy/polkit/verify.sh            # comprueba los permisos, también los de nginx
```

El instalador no sobrescribe las plantillas que ya hayas editado. Si hay otro `default_server`
(p. ej. el sitio `default` de la distribución), avisa y no instala el suyo: desactívalo para que
un host desconocido no reciba ese sitio.

En **modo local** usa dominios `*.localhost` (p. ej. `app.localhost`, que los navegadores
resuelven a 127.0.0.1) y sin HTTPS: Let's Encrypt solo emite para dominios públicos.

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
- `deploy/nginx/`: integración con nginx (ver «Sitios web con nginx»); `webhooks.conf` es el
  proxy TLS que solo expone `POST /api/webhooks/github`.
- `deploy/systemd/bbel@.service` y `bbel-deploy@.service`: plantillas que ejecutan y despliegan
  los servicios (obligatorio: ver «Servicios: plantillas de systemd»).
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
