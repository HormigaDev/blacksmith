# Cliente de escritorio de BBEL Blacksmith

Tauri 2 + SvelteKit (Svelte 5, modo SPA con `adapter-static`). Plataformas: Linux y Windows.

## Desarrollo

```sh
pnpm install
pnpm tauri dev      # aplicación de escritorio
pnpm dev            # solo el frontend, en el navegador (http://localhost:1420)
pnpm check          # tipos, reglas de estilo y formato
pnpm format
pnpm tauri build    # paquetes deb, rpm, AppImage, NSIS y MSI
```

## Estilos y temas

Lectura obligatoria antes de tocar la interfaz: [docs/styles.md](docs/styles.md). En resumen:
solo variables CSS (`src/lib/styles/`), temas claro y oscuro, y la posibilidad de añadir más
con un archivo CSS y una entrada en `src/lib/theme/themes.ts`.
