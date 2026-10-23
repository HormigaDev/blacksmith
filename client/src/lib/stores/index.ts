/**
 * Estado global de la aplicación (reglas: client/docs/state.md). Cada dato tiene un único dueño y
 * toda la interfaz reacciona a sus cambios: ninguna pantalla guarda su propia copia.
 */
export { github } from '$lib/github';
export { servers } from '$lib/servers/servers.svelte';
export { ServerServices, ServiceEntry, serviceStore } from './services.svelte';
