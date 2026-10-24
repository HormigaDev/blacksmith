# Blacksmith

[![Client checks](https://github.com/HormigaDev/blacksmith/actions/workflows/client.yml/badge.svg)](https://github.com/HormigaDev/blacksmith/actions/workflows/client.yml)
[![Rust](https://img.shields.io/badge/Rust-1.97%2B-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://tauri.app/)
[![Licencia: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)

[English](../README.md) | [Español](./README_ESPAÑOL.md) | [Português](./README_PORTUGUÊS.md)

## Índice

- [Por qué existe Blacksmith](#por-qué-existe-blacksmith)
- [Qué puede administrar](#qué-puede-administrar)
- [Qué no hace deliberadamente](#qué-no-hace-deliberadamente)
- [Arquitectura](#arquitectura)
- [Capturas de pantalla](#capturas-de-pantalla)
- [Inicio rápido para usuarios con experiencia](#inicio-rápido-para-usuarios-con-experiencia)
- [Guía completa de instalación](#guía-completa-de-instalación)
- [Desarrollo](#desarrollo)
- [Mapa del repositorio](#mapa-del-repositorio)
- [Seguridad](#seguridad)
- [Licencia](#licencia)
- [Apoya el proyecto](#apoya-el-proyecto)

Blacksmith es un pequeño panel de control de escritorio y un agente de servidor para desplegar
aplicaciones sencillas en servidores VPS bare-metal.

Está pensado para quienes quieren la comodidad de un panel de despliegue sin convertir un VPS
de 4 GB en otro proyecto de infraestructura. El agente está escrito en Rust, se ejecuta como
un servicio de systemd y permanece privado en el servidor. El cliente de escritorio se conecta
a través de un túnel SSH, por lo que la API de administración no necesita exponerse a Internet.

Blacksmith tiene un alcance intencionadamente limitado. No es una PaaS generalista, una
plataforma de contenedores ni un orquestador multi-host. Administra unas pocas aplicaciones
en un servidor bajo tu control usando las herramientas del sistema que ya existen allí.

## Por qué existe Blacksmith

Las alternativas autoalojadas inspiradas en Heroku o Vercel pueden ser útiles, pero suelen
traer sus propias bases de datos, colas, proxies, runtimes de contenedores o capas de
orquestación. En un VPS con 4 GB de RAM y una vCPU, ese consumo es el presupuesto completo.

Blacksmith se creó para resolver un problema más sencillo:

> Desplegar y operar algunos proyectos de forma fiable sin repetir manualmente los mismos
> comandos SSH en cada versión y sin instalar una plataforma pesada para hacerlo.

Rust fue elegido para el agente porque la parte que corre en el servidor debe ser pequeña,
predecible y cuidadosa con la ejecución de procesos, las credenciales, systemd y los límites
del sistema de archivos. El resultado mantiene el despliegue cerca del host y ofrece un flujo
de trabajo de escritorio práctico.

## Qué puede administrar

Blacksmith encaja bien con aplicaciones como:

- Servicios Python administrados por systemd
- Aplicaciones y APIs Node.js
- Binarios y servicios Rust
- Sitios estáticos y frontends compilados
- APIs, servicios WebSocket o una aplicación con frontend, API y WebSocket

Un despliegue puede clonar una rama, ejecutar comandos de preparación, activar una nueva
versión, iniciar el servicio y conservar la versión anterior para volver atrás. La salida
estática puede publicarse mediante la integración incluida con nginx. Los archivos de entorno,
los logs, los límites de recursos y la información básica del host se administran desde el
mismo cliente.

## Qué no hace deliberadamente

Blacksmith no usa ni administra:

- Docker, Podman, Kubernetes u otros runtimes de contenedores
- Funciones serverless
- Orquestación multi-host
- Aplicaciones distribuidas complejas
- Un endpoint público de administración
- Un reemplazo de systemd, nginx, Git u OpenSSH

La restricción bare-metal forma parte del diseño, no es una funcionalidad pendiente. Si la
aplicación necesita una malla de servicios, un planificador distribuido, escalado horizontal
automático o imágenes de contenedor aisladas, Blacksmith no es la herramienta adecuada.

## Arquitectura

El repositorio es un workspace de Cargo con cuatro partes principales:

| Componente          | Función                                                     |
| ------------------- | ----------------------------------------------------------- |
| `agent/`            | Servicio Axum y Tokio instalado en el VPS                   |
| `client/`           | Interfaz de usuario SvelteKit                               |
| `client/src-tauri/` | Runtime de escritorio Tauri 2 y transporte SSH              |
| `shared/`           | Tipos de API serializables compartidos por agente y cliente |

El agente escucha por defecto en `127.0.0.1:4040`. La aplicación de escritorio abre un
reenvío local SSH hacia esa dirección y envía las peticiones a través del túnel. OpenSSH
autentica la conexión; Blacksmith no añade un segundo sistema de inicio de sesión con JWT ni
sesiones de navegador.

La superficie HTTP pública se limita al endpoint de webhooks de GitHub cuando se habilitan.
La administración permanece detrás de SSH.

### Modelo de seguridad

- El agente no ejecuta directamente comandos de las aplicaciones; lo hacen las plantillas
  de servicios de systemd.
- En las instalaciones de servidor, los servicios de las aplicaciones usan una cuenta sin
  privilegios separada.
- systemd y polkit restringen las unidades que el agente puede controlar.
- Las credenciales de despliegue se pasan mediante credenciales de systemd, no como argumentos
  de línea de comandos ni URLs de repositorio.
- Las claves privadas permanecen en el runtime de escritorio y se guardan en el almacén de
  credenciales del sistema mediante cifrado de envoltura.
- Los procesos de servicio están confinados a su propio directorio de estado mediante el
  aislamiento de systemd.
- Los límites de recursos se aplican al cgroup del servicio, no solo a su proceso principal.

Revisa las plantillas de despliegue y las reglas de polkit antes de usar Blacksmith en un host
de producción. Forman parte de la frontera de seguridad.

## Capturas de pantalla

El repositorio incluye las capturas visuales generadas por la suite de regresión del cliente.
Estas son vistas representativas del flujo de trabajo de escritorio:

![Lista de servidores](../client/e2e/__screenshots__/visual/server-list--lista-dark.png)

![Panel del servicio](../client/e2e/__screenshots__/visual/service-dashboard--en-marcha-dark.png)

![Formulario de despliegue](../client/e2e/__screenshots__/visual/deployment-form--definido-dark.png)

![Logs en vivo](../client/e2e/__screenshots__/visual/log-viewer--en-vivo-dark.png)

La suite visual también cubre el modo claro, fallos de conexión, confirmación de claves del
host, historial de despliegues, edición del entorno, límites de recursos, sitios nginx y
servicios estáticos.

## Inicio rápido para usuarios con experiencia

### Compilar el agente

Blacksmith requiere Rust 1.97 o posterior. El repositorio fija el toolchain en
[`rust-toolchain.toml`](../rust-toolchain.toml).

```sh
git clone https://github.com/HormigaDev/blacksmith.git
cd blacksmith
cargo build --release -p agent
```

El binario resultante es `target/release/blacksmith-agent`.

### Instalar en el servidor

La versión corta, como root en el VPS y desde la raíz del repositorio (cada paso se explica en la
[guía completa](#guía-completa-de-instalación)):

```sh
apt install git openssh-client polkitd
for user in blacksmith blacksmith-apps blacksmith-fetch; do
    useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin "$user"
done
install -D -o root -g root -m 0755 target/release/blacksmith-agent /opt/blacksmith/bin/blacksmith-agent
install -d -o root -g blacksmith -m 0750 /etc/blacksmith
install -o root -g blacksmith -m 0640 deploy/agent.env /etc/blacksmith/agent.env
install -o root -g root -m 0644 deploy/polkit/10-blacksmith.rules /etc/polkit-1/rules.d/
install -m 0644 deploy/systemd/*.service deploy/blacksmith.service /etc/systemd/system/
systemctl daemon-reload
sh deploy/polkit/verify.sh && sh deploy/systemd/verify-sandbox.sh
systemctl enable --now blacksmith.service
```

Opcional: `sh deploy/nginx/install.sh` para sitios web y
`sh deploy/nginx/install-webhooks.sh <dominio>` para desplegar al hacer push. Nunca sustituyas la
regla de polkit por una entrada amplia en `sudoers`.

### Ejecutar el cliente de escritorio

```sh
cd client
pnpm install
pnpm tauri dev
```

Añade el servidor en el cliente, acepta su clave de host en la primera conexión y configura
el repositorio y la rama del servicio. Para desarrollo local sin VPS, el agente puede usar la
instancia de systemd del usuario actual:

```sh
BLACKSMITH_SYSTEMD_SCOPE=user cargo run -p agent
```

El modo local no proporciona el sandbox del servidor. Úsalo únicamente con repositorios y
comandos de confianza.

## Guía completa de instalación

Todos los comandos de los pasos 1 a 8 se ejecutan **como root en el VPS, desde la raíz de una
copia de este repositorio** (los scripts y las unidades se leen de `deploy/`).

### Requisitos

En el VPS (Debian 12+ o Ubuntu 22.04+; en otras distribuciones los paquetes pueden llamarse
distinto):

- systemd, polkit (`polkitd`), Git y el cliente de OpenSSH (`git`, `openssh-client`): el agente
  gestiona las unidades mediante polkit y los despliegues clonan con `git` (y `ssh` para las
  llaves de despliegue).
- Un servidor SSH que permita el reenvío TCP (ver el paso 6).
- Para sitios web: `nginx` y, para HTTPS, `certbot`. Un dominio cuyo DNS apunte al VPS y los
  puertos 80 y 443 abiertos.
- Para compilar en el propio VPS: Rust 1.97+ y un compilador de C (`build-essential`).

En el escritorio:

- Rust 1.97 o superior
- Node.js 24.14.0
- pnpm 11
- Los requisitos de sistema de Tauri 2 para tu plataforma (en Linux, los paquetes de desarrollo
  de GTK 3 y WebKitGTK 4.1 de la [guía de requisitos de Tauri](https://v2.tauri.app/start/prerequisites/))

```sh
apt install git openssh-client polkitd
```

### 1. Compilar el agente

```sh
cargo build --release -p agent
```

Compila en el VPS, o en una máquina con la misma arquitectura y una glibc igual o más antigua, y
copia `target/release/blacksmith-agent` junto con el directorio `deploy/`.

### 2. Crear las cuentas de servicio

Tres cuentas, para que ningún código desplegado se ejecute con los permisos del agente:

| Cuenta | Ejecuta | Puede |
|---|---|---|
| `blacksmith` | el agente | gestionar sus propias unidades mediante polkit |
| `blacksmith-apps` | los servicios y sus comandos de preparación | escribir solo en el directorio de su servicio |
| `blacksmith-fetch` | el clon del repositorio | leer las credenciales del despliegue, nada más |

```sh
for user in blacksmith blacksmith-apps blacksmith-fetch; do
    useradd --system --no-create-home --home-dir /nonexistent \
        --shell /usr/sbin/nologin "$user"
done
```

### 3. Instalar el agente y su configuración

```sh
install -D -o root -g root -m 0755 target/release/blacksmith-agent /opt/blacksmith/bin/blacksmith-agent
install -d -o root -g blacksmith -m 0750 /etc/blacksmith
install -o root -g blacksmith -m 0640 deploy/agent.env /etc/blacksmith/agent.env
```

El binario pertenece a root, así el agente nunca puede sustituir su propio ejecutable.
`/etc/blacksmith/agent.env` tiene que existir aunque todas sus líneas estén comentadas: los
valores por defecto son los de producción. Admite las variables `BLACKSMITH_*` documentadas en
[`.env.template`](../.env.template); una desconocida impide arrancar al agente. Los secretos
nunca van en este archivo.

### 4. Instalar las plantillas de servicio y polkit

```sh
install -m 0644 deploy/systemd/*.service /etc/systemd/system/
install -m 0644 deploy/blacksmith.service /etc/systemd/system/
install -o root -g root -m 0644 deploy/polkit/10-blacksmith.rules /etc/polkit-1/rules.d/
systemctl daemon-reload
```

Comprueba ambas cosas antes de arrancar el agente:

```sh
sh deploy/polkit/verify.sh            # termina con «regla correcta»
sh deploy/systemd/verify-sandbox.sh   # termina con «Sandbox correcto.»
```

`verify.sh` crea unidades temporales y comprueba lo que el agente puede y no puede hacer.
`verify-sandbox.sh` arranca un servicio desechable con el mismo sandbox que uno real y comprueba
que no ve otros servicios, los datos del agente, D-Bus ni procesos de otros usuarios. Los dos
limpian lo que crean.

### 5. Arrancar el agente

```sh
systemctl enable --now blacksmith.service
systemctl status blacksmith.service
journalctl -u blacksmith.service -n 50
```

El agente solo escucha en `127.0.0.1:4040` y al arrancar registra si polkit lo autoriza. Su base
de datos se crea en `/var/lib/blacksmith/agent.db`. No abras el puerto 4040 en el cortafuegos:
nada fuera del servidor lo necesita.

### 6. Permitir el túnel SSH

El cliente de escritorio llega al agente con un túnel SSH hacia `127.0.0.1:4040`, usando la llave
SSH de cualquier cuenta del VPS (no hace falta root). El servidor SSH debe permitir el reenvío
TCP para esa cuenta. Lo permite por defecto; si `/etc/ssh/sshd_config` (o un archivo de
`sshd_config.d/`) tiene `AllowTcpForwarding no`, ponlo en `local` para esa cuenta, por ejemplo:

```text
Match User deploy
    AllowTcpForwarding local
```

y `systemctl reload ssh`. En la primera conexión el cliente muestra la huella de la llave del
servidor; compárala con `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` antes de aceptarla.

### 7. Sitios web con nginx (opcional)

Solo hace falta para servicios que sirven un sitio web (archivos estáticos o un proxy inverso):

```sh
apt install nginx certbot
sh deploy/nginx/install.sh
```

El script instala la configuración común, los snippets de seguridad, las plantillas editables en
`/etc/blacksmith/nginx/templates/` y las dos unidades que corren como root en nombre del agente
(validar y recargar nginx, pedir certificados), cada una protegida por un script que vuelve a
validar lo que escribió el agente. No necesita ningún dominio y puede repetirse tras cada
actualización.

Si avisa de que ya hay otro `default_server` (normalmente el sitio `default` de Debian), quítalo
para que los hosts desconocidos no reciban ese sitio y vuelve a ejecutar el script:

```sh
rm /etc/nginx/sites-enabled/default
sh deploy/nginx/install.sh
```

Los dominios se configuran por servicio, en la pestaña **Web** del cliente. Antes de guardar un
sitio, apunta el DNS del dominio (registros A/AAAA) al VPS y abre los puertos 80 y 443. Con HTTPS
activado, el certificado se pide en segundo plano y lo renueva el temporizador propio de certbot.

### 8. Desplegar al hacer push con webhooks de GitHub (opcional)

Requiere el paso 7 y un dominio propio para los webhooks (por ejemplo `hooks.ejemplo.com`) cuyo
DNS ya apunte al VPS:

```sh
sh deploy/nginx/install-webhooks.sh hooks.ejemplo.com tu@ejemplo.com
```

El script genera el secreto del webhook (solo legible por root, que el agente recibe como
credencial de systemd mediante un drop-in, así que nunca editas la unidad), obtiene el
certificado del dominio, publica solo `POST /api/webhooks/github` y muestra los valores que hay
que poner en GitHub. Después, en cada repositorio: **Settings → Webhooks → Add webhook**:

- **Payload URL**: `https://hooks.ejemplo.com/api/webhooks/github`
- **Content type**: `application/json`
- **Secret**: el valor que muestra el script (también en `/etc/blacksmith/credentials/github-webhook-secret`)
- **Events**: *Just the push event*

Cada push a una rama despliega todos los servicios configurados con ese repositorio y esa rama
que tengan activado **Desplegar al hacer push** (en la pestaña **Despliegue**); viene activado
por defecto. Las peticiones con una firma inválida se rechazan; la API de administración nunca
se publica.

Si usas una GitHub App propia en lugar de webhooks de repositorio, pon la misma URL y el mismo
secreto en su configuración de webhook. La App por defecto de BBEL Blacksmith no sirve para esto,
porque no controlas su webhook. Blacksmith nunca necesita la llave privada de ninguna GitHub App.

Cuando un servicio usa una llave de despliegue, una GitHub App propia con **Administration: Read
and write** registra automáticamente la llave de solo lectura generada. Con la App por defecto de
BBEL Blacksmith el cliente pasa al flujo manual de copiar y pegar, porque ese permiso no se le
concede a propósito.

### 9. Registrar y desplegar un servicio

Desde el cliente de escritorio:

1. Añade el VPS y conéctate por SSH.
2. Registra un servicio con un identificador en minúsculas.
3. Elige el repositorio de GitHub y la rama.
4. Añade comandos de preparación como `python -m venv .venv`, `npm ci` o `cargo build --release`.
5. Define el comando de arranque del servicio, si tiene proceso.
6. Opcionalmente, elige una carpeta pública para un sitio estático y configura su dominio en la
   pestaña **Web**.
7. Despliega y revisa el estado y los logs del servicio.

Cada despliegue tiene su propio directorio de versión. La versión anterior sigue disponible para
volver a ella si una compilación o un arranque nuevos fallan.

### Actualizar

Vuelve a compilar el agente y repite las instalaciones, que son idempotentes:

```sh
install -o root -g root -m 0755 target/release/blacksmith-agent /opt/blacksmith/bin/blacksmith-agent
install -m 0644 deploy/systemd/*.service deploy/blacksmith.service /etc/systemd/system/
install -o root -g root -m 0644 deploy/polkit/10-blacksmith.rules /etc/polkit-1/rules.d/
systemctl daemon-reload && systemctl restart blacksmith.service
sh deploy/nginx/install.sh                                  # si usas sitios web
sh deploy/nginx/install-webhooks.sh hooks.ejemplo.com       # si usas webhooks
```

La base de datos se migra sola. Se conservan tus plantillas de nginx editadas y el secreto del
webhook.

## Desarrollo

Desde la raíz del repositorio:

```sh
cargo check --workspace
cargo test -p agent -p shared
cargo clippy -p agent -p shared --all-targets -- -D warnings
```

Para el cliente:

```sh
cd client
pnpm install --frozen-lockfile
pnpm check
pnpm test
pnpm test:e2e:app
```

El flujo del cliente se ejecuta manualmente desde GitHub Actions porque sus suites de navegador
y regresión visual son deliberadamente más pesadas que una comprobación por commit.

## Mapa del repositorio

```text
agent/                 Agente VPS e integración con systemd
client/                Interfaz SvelteKit, tests y capturas visuales
client/src-tauri/      Runtime Tauri de escritorio e implementación SSH
shared/                Modelos de API compartidos de Rust
deploy/                Archivos de systemd, nginx, polkit e instalación
```

## Seguridad

Blacksmith requiere conocimientos cuidadosos de administración de sistemas. Lee la
[política de seguridad](../SECURITY.md) antes de desplegarlo en un host de producción. Allí se
explican las versiones `v1.0.x` con soporte, el proceso privado para reportar vulnerabilidades
y la responsabilidad del operador de configurar correctamente SSH, systemd, polkit,
credenciales, firewall y comandos de despliegue.

## Licencia

Blacksmith se distribuye bajo la [Licencia MIT](../LICENSE).

## Apoya el proyecto

Si Blacksmith te ahorra tiempo o hace más práctico un VPS pequeño, puedes apoyar su
mantenimiento continuo:

[![Donar con PayPal](https://www.paypalobjects.com/en_US/i/btn/btn_donateCC_LG.gif)](https://www.paypal.com/donate/?hosted_button_id=UCL7EE2G44KPQ)
