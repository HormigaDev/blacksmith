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

| Componente | Función |
| --- | --- |
| `agent/` | Servicio Axum y Tokio instalado en el VPS |
| `client/` | Interfaz de usuario SvelteKit |
| `client/src-tauri/` | Runtime de escritorio Tauri 2 y transporte SSH |
| `shared/` | Tipos de API serializables compartidos por agente y cliente |

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

### Instalar las unidades del servidor

Instala el agente y las plantillas de systemd como root en el VPS. Las rutas exactas se pueden
adaptar al host, pero la cuenta de servicio y los nombres de las unidades deben conservar la
coherencia con los archivos de `deploy/`.

```sh
install -D -o root -g root -m 0755 \
  target/release/blacksmith-agent \
  /opt/blacksmith/bin/blacksmith-agent

install -m 0644 deploy/systemd/*.service /etc/systemd/system/
install -m 0644 deploy/blacksmith.service /etc/systemd/system/
install -m 0644 deploy/polkit/10-blacksmith.rules \
  /etc/polkit-1/rules.d/10-blacksmith.rules

systemctl daemon-reload
systemctl enable --now blacksmith.service
```

La cuenta de servicio, los directorios, el archivo de entorno, la integración con nginx y las
credenciales deben prepararse según los archivos de despliegue y la guía completa. No
sustituyas las reglas de polkit por una entrada amplia de `sudoers`.

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

### Requisitos

En el VPS:

- Un sistema Linux con systemd
- Git
- El agente Blacksmith compilado con Rust
- polkit
- nginx y soporte de ACL cuando se sirvan sitios estáticos o proxies inversos
- certbot cuando sean necesarios certificados HTTPS

En el escritorio:

- Rust 1.97 o posterior
- Node.js 24.14.0
- pnpm 11
- Los prerrequisitos de Tauri 2 para tu plataforma

Para desarrollo de escritorio en Linux, instala los paquetes de desarrollo GTK 3 y WebKitGTK
4.1 indicados en la [guía de prerrequisitos de Tauri](https://v2.tauri.app/start/prerequisites/).

### 1. Crear las cuentas de servicio

Las instalaciones de servidor usan cuentas separadas para el agente, los servicios de
aplicaciones y las operaciones de descarga. Créelas una vez como root:

```sh
for user in blacksmith blacksmith-apps blacksmith-fetch; do
    useradd --system --no-create-home --home-dir /nonexistent \
        --shell /usr/sbin/nologin "$user"
done
```

Las unidades versionadas en `deploy/systemd/` documentan la propiedad y las rutas que esperan.

### 2. Instalar y verificar polkit

```sh
install -o root -g root -m 0644 \
  deploy/polkit/10-blacksmith.rules \
  /etc/polkit-1/rules.d/10-blacksmith.rules

sh deploy/polkit/verify.sh
```

El script de verificación crea unidades de prueba temporales y comprueba operaciones permitidas
y denegadas. Un resultado correcto debe terminar con `regla correcta`.

### 3. Configurar el agente

Copia la plantilla de entorno y establece los valores adecuados para el host:

```sh
install -d -o root -g blacksmith -m 0750 /etc/blacksmith
install -o root -g blacksmith -m 0640 agent.env /etc/blacksmith/agent.env
```

La configuración usa variables `BLACKSMITH_*`. El agente valida las variables desconocidas y
los valores fuera de rango al arrancar. En producción, los secretos como el secreto del
webhook deben proporcionarse mediante credenciales de systemd y no como variables de entorno
normales.

### 4. Instalar la integración con nginx cuando sea necesario

Para sitios y proxies inversos:

```sh
apt install nginx certbot acl
sh deploy/nginx/install.sh
```

El agente genera la configuración de nginx a partir de datos de sitios validados. Admite
raíces estáticas, upstreams HTTP, WebSockets y HTTPS opcional mediante certbot. La API de
administración no se publica con esta configuración.

### 5. Registrar y desplegar un servicio

Desde el cliente de escritorio:

1. Añade el VPS y conéctate mediante SSH.
2. Registra un servicio con un slug en minúsculas.
3. Selecciona el repositorio y la rama de GitHub.
4. Añade comandos de preparación como `python -m venv .venv`, `npm ci` o
   `cargo build --release`.
5. Define el comando de inicio del servicio si tiene un proceso.
6. Elige opcionalmente un directorio público para un sitio estático.
7. Despliega y revisa el estado y los logs del servicio.

Cada despliegue recibe su propio directorio de versión. La versión anterior permanece
disponible para volver atrás si falla una compilación o el inicio del servicio.

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
