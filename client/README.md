# Cliente de escritorio de Blacksmith

Tauri 2 + SvelteKit (Svelte 5, modo SPA con `adapter-static`). Plataformas: Linux y Windows.

## Desarrollo

```sh
pnpm install
pnpm tauri dev      # aplicación de escritorio
pnpm dev            # solo el frontend, en el navegador (http://localhost:1420)
pnpm workbench      # componentes aislados, en una ventana
pnpm check          # tipos, estilos, i18n, tests y formato
pnpm test:e2e       # E2E, accesibilidad (Playwright)
pnpm format
pnpm tauri build    # paquete local según la plataforma y los targets configurados
```

## Paquetes de release

La release oficial se genera desde un tag SemVer, por ejemplo `v1.0.0`. El paquete Debian
conserva el nombre interno `blacksmith`, por lo que `dpkg` puede actualizar una instalación
anterior cuando se instala una versión posterior:

```sh
cd client
pnpm tauri build --target x86_64-unknown-linux-gnu --bundles deb
sudo dpkg -i ../target/x86_64-unknown-linux-gnu/release/bundle/deb/*.deb
```

Objetivos oficiales:

| Plataforma  | Target Rust                 | Artefactos publicados                       |
| ----------- | --------------------------- | ------------------------------------------- |
| Linux x64   | `x86_64-unknown-linux-gnu`  | `linux-blacksmith-v<version>.deb`           |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `linux-arm-blacksmith-v<version>.deb`       |
| Windows x64 | `x86_64-pc-windows-msvc`    | `windows-blacksmith-v<version>.exe`, `.msi` |

Los instaladores oficiales se generan automáticamente al publicar un tag `v*.*.*` y se suben
a las releases de GitHub. No se generan objetivos x32.

## Documentación

- [docs/styles.md](docs/styles.md): tokens, temas y estilos.
- [docs/i18n.md](docs/i18n.md): textos e idiomas.
- [docs/components.md](docs/components.md): componentes, HTTP y dependencias.
- [docs/testing.md](docs/testing.md): tests y Workbench.

## Estilos y temas

Lectura obligatoria antes de tocar la interfaz: [docs/styles.md](docs/styles.md). En resumen:
solo variables CSS (`src/lib/styles/`), temas claro y oscuro, y la posibilidad de añadir más
con un archivo CSS y una entrada en `src/lib/theme/themes.ts`.
