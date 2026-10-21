# BBEL Blacksmith - Contexto del Proyecto

## Visión General

BBEL Blacksmith es un panel de gestión _Zero-Trust_ para proyectos desplegados en servidores VPS (bare-metal). Permite administrar servicios nativos gestionados por `systemd`, visualizar métricas de recursos (CPU/RAM) y logs en tiempo real, inyectar variables de entorno de forma segura y controlar despliegues automáticos o manuales a través de ramas de GitHub.

## Arquitectura "Zero-Trust" (Core Concept)

El sistema rechaza la exposición pública de puertos de administración.

1. **Agent (VPS):** Un binario ultraligero que corre en el servidor escuchando estrictamente en `127.0.0.1:4040`.
2. **Client (Desktop):** La aplicación cliente abre un **túnel SSH** en segundo plano contra el VPS usando las llaves del usuario, mapeando el puerto 4040 local al 4040 del servidor. No existe sistema de login (JWT/Cookies); la autenticación se delega a OpenSSH.
3. **Webhooks:** La única ruta accesible desde internet es `/api/webhooks` (expuesta a través de un proxy inverso estricto en Nginx) para recibir eventos de GitHub.

## Stack Tecnológico

- **Estructura:** Monorepo gestionado con Cargo Workspaces (`resolver = "2"`).
- **Agent (Backend):** Rust, Axum, Tokio, wrappers nativos para `systemctl` y `journalctl`, y `sysinfo` para telemetría.
- **Client UI (Frontend):** Svelte + TypeScript.
- **Client App (Desktop):** Tauri v2 (Rust).
- **Shared:** Librería Rust para compartir tipos de datos estandarizados (structs/enums) entre el Agent y el motor de Tauri.

## Estructura de Directorios

- `/agent/` -> Backend Axum que se despliega en el VPS.
- `/client/` -> UI en Svelte.
- `/client/src-tauri/` -> Motor Rust de la aplicación de escritorio.
- `/shared/` -> Crate de librería con estructuras serializables compartidas.

## Filosofía de Desarrollo y Reglas

1. **Sin Contenedores:** Asumimos despliegues "bare-metal". No uses ni sugieras Docker, Podman o Kubernetes. El agente maneja procesos del sistema directamente.
2. **Eficiencia Extrema:** El Agente debe mantenerse por debajo de los 20MB de RAM en reposo. Usa código asíncrono no bloqueante y evita clonar datos innecesariamente en Rust.
3. **Seguridad Absoluta:** Cualquier acción destructiva en el sistema de archivos o systemd debe ser validada minuciosamente. Las variables de entorno (`.env`) se escriben con permisos restringidos de Linux.
4. **Frontend (client):** Antes de crear o modificar cualquier página, componente o estilo en `/client/`, lee y sigue **[client/docs/styles.md](client/docs/styles.md)**. Es obligatorio: nunca se escriben colores ni valores de identidad visual (espaciado, tipografía, radios, sombras, capas, duraciones); siempre variables CSS de `src/lib/styles/`. Los componentes usan Svelte 5 con runes y snippets. Un cambio no está terminado hasta que `pnpm check` (en `/client/`) pasa sin errores.
5. **Internacionalización (client):** Todo texto visible para el usuario se escribe con `t()` de `$lib/i18n`, nunca a mano en un componente. Las reglas están en **[client/docs/i18n.md](client/docs/i18n.md)**, de lectura obligatoria. El español (`src/lib/i18n/messages/es.ts`) es el idioma base y la fuente de verdad de las claves: toda clave nueva se añade ahí primero. Inglés y portugués están preparados y, mientras no se traduzcan, muestran el español. Números, bytes y fechas se formatean con las funciones de `$lib/i18n` y los errores del agente se traducen por su `code`, nunca por su `message`.
6. **Componentes, HTTP y dependencias (client):** Toda pantalla se construye con los componentes de `$lib/components/ui` (nunca `<input>`, `<select>`, `<textarea>` ni botones nativos en las páginas) y cualquier acción destructiva se confirma con `ConfirmDialog`. Las peticiones HTTP usan `$lib/http` (Fetch vanilla, sin Axios); la webview no llama al agente directamente: se inyecta un transporte que se ejecuta en Rust, sin relajar la CSP ni la guarda del agente. Las dependencias npm se fijan en versiones exactas y con al menos 7 días de antigüedad (`client/pnpm-workspace.yaml`). Las reglas completas están en **[client/docs/components.md](client/docs/components.md)**, de lectura obligatoria.
7. **Acoplamiento de Tipos:** Si se modifica un endpoint en el Agente, sus estructuras de datos (payloads/responses) deben modificarse en el crate `/shared/` para que el compilador alerte de los cambios al Cliente (Tauri). Si se añade un `ErrorCode`, se refleja también en `API_ERROR_CODES` (`client/src/lib/http/client.ts`) y en `errors.agent.<CODE>` de los mensajes.
