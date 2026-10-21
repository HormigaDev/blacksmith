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
4. **Acoplamiento de Tipos:** Si se modifica un endpoint en el Agente, sus estructuras de datos (payloads/responses) deben modificarse en el crate `/shared/` para que el compilador alerte de los cambios al Cliente (Tauri).
