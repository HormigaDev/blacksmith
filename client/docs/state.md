# Estado global del cliente

La interfaz no guarda copias propias de datos compartidos: cada dato tiene **un único dueño** (un
_store_) y las pantallas leen de él. Así, cuando algo cambia en un sitio (se conecta GitHub, se
guarda un despliegue, un servicio arranca), todas las vistas lo reflejan al momento.

## 1. Cómo es un store

- Una **clase con runes** (`$state`, `$derived`) en un archivo `*.svelte.ts`, y una instancia
  exportada. No se usa `svelte/store` (`writable`, `$store`) ni librerías externas: las runes ya dan
  reactividad fina, tipada y sin suscripciones manuales.
- Se importa desde `$lib/stores`, el punto de entrada único:

    ```ts
    import { github, servers, serviceStore } from '$lib/stores';
    ```

- Los campos son públicos y reactivos; las **acciones** son métodos (`load`, `save…`, `connect`).
  Una pantalla no escribe en los campos de otro dueño salvo los de interfaz pensados para ello
  (p. ej. `github.panelOpen`).
- **Errores**: las cargas guardan el error traducido en el propio store (`error`), para mostrarlo
  donde toque; las acciones **lanzan**, para que el formulario que las pidió enseñe el motivo.
- Los componentes de `$lib/components` **no importan stores**: reciben datos y callbacks por props
  (se prueban y se ven en el Workbench sin estado global). Quien conecta store y componente es la
  página (`src/routes`) o un componente de layout (`AppShell`, `GitHubMenu`).

## 2. Stores existentes

| Store          | Dueño de                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `servers`      | Servidores guardados, su orden y vista, el estado de cada túnel y la huella/passphrase pendiente de confirmar.                                                                                                                                                                                                                                                                                                                                                       |
| `github`       | App de GitHub en uso (la del proyecto o una propia), cuenta conectada, Device Flow en curso, repositorios y si el panel de conexión está abierto.                                                                                                                                                                                                                                                                                                                    |
| `serviceStore` | Por servidor (`server(id)`): lista de servicios, variables globales y datos del servidor (`host`: sistema, hardware y uso; `loadHost`). Por servidor, además, la cuenta de GitHub con la que descarga repositorios privados (`shareGitHub` la renueva al guardar y al desplegar). Por servicio (`service(id, nombre)`): estado, despliegue, variables, llave de despliegue (si la usa), sitio de nginx, límites de recursos (`limits`) y despliegues (con rollback). |

- `servers` y `github` se cargan **una vez** en `src/routes/(app)/+layout.svelte`. Los diálogos de
  confirmar huella y pedir passphrase también viven ahí: se puede conectar desde cualquier pantalla.
- `serviceStore` crea las entradas bajo demanda y las conserva mientras la app esté abierta: volver
  a una pantalla muestra lo último conocido al instante y refresca por detrás. Al desconectar o
  quitar un servidor se descartan (`serviceStore.drop(id)`).

## 3. Refresco periódico

`ServiceEntry.watch()` refresca el estado del servicio cada 3 s (2 s durante un despliegue) y
devuelve la función que lo detiene. Lleva un **contador de vistas**: varias pantallas pueden
observar el mismo servicio sin multiplicar peticiones, y el temporizador se para cuando la última
deja de mirar. Se usa dentro de un `$effect`, que lo detiene solo al salir:

```svelte
$effect(() => service.watch());
```

## 4. Añadir un store

1. Clase con runes en `src/lib/<área>/<nombre>.svelte.ts` (o en `src/lib/stores/` si no pertenece
   a un área concreta) y una instancia exportada.
2. Re-exportarla en `src/lib/stores/index.ts`.
3. Tests de su lógica en `*.test.ts` junto a él, con el transporte (Tauri / agente) simulado.
