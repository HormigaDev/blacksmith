-- Versión 12: desplegar el servicio al recibir un push a su rama (webhook de GitHub).
ALTER TABLE deployments ADD COLUMN auto_deploy INTEGER NOT NULL DEFAULT 1 CHECK (auto_deploy IN (0, 1));
