-- Límites de recursos de cada servicio (versión 11 del esquema; ver `MIGRATIONS` en db/mod.rs).
-- Sin fila, el servicio no tiene límites. Mismas reglas que `limits::validate`; NULL = sin límite.
CREATE TABLE service_limits (
    service_id  INTEGER PRIMARY KEY REFERENCES services (id) ON DELETE CASCADE,
    -- CPUQuota en %: 100 = un núcleo.
    cpu_percent INTEGER CHECK (cpu_percent IS NULL OR cpu_percent >= 1),
    priority    TEXT    NOT NULL DEFAULT 'normal' CHECK (priority IN ('low', 'normal', 'high')),
    memory_max  INTEGER CHECK (memory_max IS NULL OR memory_max > 0),
    -- 0 = sin swap.
    swap_max    INTEGER CHECK (swap_max IS NULL OR swap_max >= 0),
    tasks_max   INTEGER CHECK (tasks_max IS NULL OR tasks_max >= 1),
    updated_at  INTEGER NOT NULL
) STRICT;
