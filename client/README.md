# Cliente de escritorio de BBEL Blacksmith

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
pnpm tauri build    # paquetes deb, rpm, AppImage, NSIS y MSI
```

## Documentación

- [docs/styles.md](docs/styles.md): tokens, temas y estilos.
- [docs/i18n.md](docs/i18n.md): textos e idiomas.
- [docs/components.md](docs/components.md): componentes, HTTP y dependencias.
- [docs/testing.md](docs/testing.md): tests y Workbench.

## Estilos y temas

Lectura obligatoria antes de tocar la interfaz: [docs/styles.md](docs/styles.md). En resumen:
solo variables CSS (`src/lib/styles/`), temas claro y oscuro, y la posibilidad de añadir más
con un archivo CSS y una entrada en `src/lib/theme/themes.ts`.
