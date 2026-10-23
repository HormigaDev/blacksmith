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
                   -- Reservado (PERMISSION_PROBE_SLUG).
                   AND slug <> 'agent-permission-check'
               ),
    name       TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    -- Informativa. Misma regla que `ServiceDescription`.
    description TEXT   NOT NULL DEFAULT '' CHECK (length(description) <= 500),
    -- Debe estar en marcha: el agente lo arranca al iniciarse (p. ej. tras reiniciar el
    -- servidor). Se activa al iniciarlo o desplegarlo y se desactiva al detenerlo a mano.
    autostart  INTEGER NOT NULL DEFAULT 0 CHECK (autostart IN (0, 1)),
    -- Versión activa y la anterior (ids `r<segundos>-<nanos>`, ver deploy::spec). La anterior es
    -- la del rollback; el runner nunca la borra al podar.
    active_release   TEXT CHECK (active_release IS NULL OR length(active_release) = 21),
    previous_release TEXT CHECK (previous_release IS NULL OR length(previous_release) = 21),
    -- Segundos Unix (UTC).
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

-- Definición del despliegue de cada servicio. Se borra con el servicio.
CREATE TABLE deployments (
    service_id     INTEGER PRIMARY KEY REFERENCES services (id) ON DELETE CASCADE,
    -- Repositorio de GitHub: los tres o ninguno.
    repo_owner     TEXT,
    repo_name      TEXT,
    repo_branch    TEXT,
    -- Cómo se descarga: 'account' (HTTPS, con la cuenta de GitHub del servidor si hace falta) o
    -- 'deployKey' (SSH, con la llave de despliegue del servicio).
    access         TEXT    NOT NULL DEFAULT 'account' CHECK (access IN ('account', 'deployKey')),
    -- Array JSON de comandos, en orden. Mismas reglas que `services::deployment`.
    setup_commands TEXT    NOT NULL DEFAULT '[]' CHECK (json_valid(setup_commands) AND json_type(setup_commands) = 'array'),
    start_command  TEXT    NOT NULL DEFAULT '',
    -- Carpeta del repositorio que se publica para nginx (sitios estáticos). Misma regla que
    -- `services::deployment`.
    publish_dir    TEXT    CHECK (publish_dir IS NULL OR length(publish_dir) BETWEEN 1 AND 200),
    updated_at     INTEGER NOT NULL,
    CHECK ((repo_owner IS NULL) = (repo_name IS NULL) AND (repo_name IS NULL) = (repo_branch IS NULL))
) STRICT;

-- Historial de despliegues (los últimos se muestran en la interfaz). Se borra con el servicio.
CREATE TABLE deploy_runs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id  INTEGER NOT NULL REFERENCES services (id) ON DELETE CASCADE,
    status      TEXT    NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    -- 'deploy': versión nueva; 'rollback': vuelta a la anterior.
    kind        TEXT    NOT NULL CHECK (kind IN ('deploy', 'rollback')),
    release     TEXT    NOT NULL CHECK (length(release) = 21),
    branch      TEXT    NOT NULL,
    started_at  INTEGER NOT NULL,
    finished_at INTEGER,
    detail      TEXT
) STRICT;

CREATE INDEX deploy_runs_by_service ON deploy_runs (service_id, id DESC);

-- Sitio web de un servicio detrás de nginx (ver nginx::sites). Se borra con el servicio.
CREATE TABLE sites (
    service_id INTEGER PRIMARY KEY REFERENCES services (id) ON DELETE CASCADE,
    template   TEXT    NOT NULL CHECK (template IN ('static', 'spa', 'app', 'api', 'websocket', 'fullstack')),
    -- Array JSON de dominios (el primero es el principal). Cada uno está además en site_domains.
    domains    TEXT    NOT NULL CHECK (json_valid(domains) AND json_type(domains) = 'array'),
    port       INTEGER CHECK (port IS NULL OR port BETWEEN 1 AND 65535),
    https      INTEGER NOT NULL CHECK (https IN (0, 1)),
    email      TEXT    CHECK (email IS NULL OR length(email) BETWEEN 3 AND 254),
    tls        TEXT    NOT NULL DEFAULT 'off' CHECK (tls IN ('off', 'issuing', 'active', 'failed')),
    tls_detail TEXT,
    updated_at INTEGER NOT NULL
) STRICT;

-- Un dominio pertenece a un solo servicio: la clave primaria lo garantiza aunque dos peticiones
-- lleguen a la vez.
CREATE TABLE site_domains (
    domain     TEXT    PRIMARY KEY CHECK (length(domain) BETWEEN 1 AND 253),
    service_id INTEGER NOT NULL REFERENCES services (id) ON DELETE CASCADE
) STRICT;
