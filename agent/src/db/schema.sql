-- Esquema de la base de datos del agente (versión en `SCHEMA_VERSION`, db/mod.rs).
--
-- Sin migraciones por ahora: si cambia el esquema, se sube la versión y la base de datos se
-- crea de nuevo. El agente se niega a arrancar con una versión distinta a la suya.
--
-- Tablas STRICT: SQLite rechaza valores de otro tipo en lugar de convertirlos.

-- Servicios registrados: lo único que el cliente puede ver y gestionar.
CREATE TABLE services (
    -- AUTOINCREMENT: un id borrado no se reutiliza nunca.
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Define la unidad: bbel-<slug>.service. Misma regla que `ServiceSlug`.
    slug       TEXT    NOT NULL UNIQUE CHECK (
                   length(slug) BETWEEN 1 AND 48
                   AND slug NOT GLOB '*[^a-z0-9-]*'
                   AND slug NOT GLOB '-*'
                   AND slug NOT GLOB '*-'
               ),
    name       TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    -- Segundos Unix (UTC).
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
