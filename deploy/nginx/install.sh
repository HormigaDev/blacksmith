#!/bin/sh
# Instala la integración de nginx de Blacksmith. Ejecutar como root desde la raíz del
# repositorio:
#   sh deploy/nginx/install.sh            servidor: el agente corre como `blacksmith`
#   sh deploy/nginx/install.sh --local    modo local: el agente corre como su usuario (con
#                                         privilegios de root obtenidos desde ese usuario)
#
# Qué hace (es idempotente; repítalo tras actualizar Blacksmith):
#   - /etc/nginx/conf.d/blacksmith.conf: ajustes comunes e `include` de los sitios que genera el agente.
#   - /etc/nginx/conf.d/blacksmith-default.conf: cierra las peticiones a hosts desconocidos (solo si no
#     hay ya otro `default_server`; ver el aviso).
#   - /etc/nginx/snippets/blacksmith/: TLS, cabeceras de seguridad, proxy, WebSocket, estáticos, ACME.
#   - /etc/blacksmith/nginx/templates/: plantillas por tipo de proyecto. Las que ya existan NO se
#     sobrescriben: son suyas para editarlas.
#   - /var/lib/blacksmith-acme: desafíos de Let's Encrypt.
#   - blacksmith-nginx.service y blacksmith-certbot@.service: lo único que corre como root por cuenta del
#     agente (validar/recargar nginx y pedir certificados), con sus scripts en /usr/local/lib/blacksmith
#     (check-sites.sh, certbot.sh), y la regla de polkit que se lo permite.
set -eu

[ "$(id -u)" -eq 0 ] || { echo "ejecútelo como root" >&2; exit 1; }
HERE=$(cd "$(dirname "$0")" && pwd)
command -v nginx >/dev/null || { echo "nginx no está instalado (apt install nginx)" >&2; exit 1; }

MODE=server
[ "${1:-}" = "--local" ] && MODE=local

if [ "$MODE" = local ]; then
    # Usuario que pidió los privilegios (sudo, doas o run0 dejan su nombre en el entorno).
    AGENT_USER=${SUDO_USER:-${DOAS_USER:-${RUN0_USER:-}}}
    [ -n "$AGENT_USER" ] && [ "$AGENT_USER" != root ] || {
        echo "modo local: ejecútelo con privilegios de root desde su usuario normal" >&2
        exit 1
    }
    HOME_DIR=$(getent passwd "$AGENT_USER" | cut -d: -f6)
    STATE=$HOME_DIR/.local/state
    # Si faltan, se crean del usuario: `install -d` crearía los padres como root.
    for dir in "$HOME_DIR/.local" "$STATE"; do
        [ -d "$dir" ] || install -d -m 0700 -o "$AGENT_USER" "$dir"
    done
else
    AGENT_USER=blacksmith
    STATE=/var/lib
fi
SITES_DIR=$STATE/blacksmith/nginx/sites
CERTS_DIR=$STATE/blacksmith/nginx/certs
APPS_ROOT=$STATE/blacksmith-apps
# Las rutas van a `sed` y a unidades de systemd: nada que pueda romperlos.
case "$STATE" in
    *[!A-Za-z0-9/._-]*) echo "ruta no admitida (solo letras, dígitos y . _ - /): $STATE" >&2; exit 1 ;;
esac

echo "==> Modo $MODE (agente: $AGENT_USER)"

# Directorios del agente (también los crea él al arrancar).
install -d -m 0700 -o "$AGENT_USER" "$STATE/blacksmith" "$STATE/blacksmith/nginx" "$SITES_DIR" "$CERTS_DIR"
install -d -m 0755 /var/lib/blacksmith-acme

echo "==> Snippets y ajustes comunes"
install -d -m 0755 /etc/nginx/snippets/blacksmith
install -m 0644 "$HERE"/snippets/blacksmith/*.conf /etc/nginx/snippets/blacksmith/
# Se instala al final, tras validarla (ver «Validando»): con un error, nginx seguiría con la anterior.
COMMON=$(mktemp)
sed "s|@SITES_DIR@|$SITES_DIR|" "$HERE/conf.d/blacksmith.conf" >"$COMMON"
# Solo puede ir en `http`, donde repetirla es un error: si su configuración ya la define, se
# respeta la suya.
if grep -rqsE '^[[:space:]]*server_names_hash_bucket_size' /etc/nginx/nginx.conf /etc/nginx/conf.d \
    --exclude=blacksmith.conf; then
    sed -i '/^server_names_hash_bucket_size/d' "$COMMON"
fi

if grep -rqsE 'listen[^;]*default_server' /etc/nginx/sites-enabled /etc/nginx/conf.d --exclude=blacksmith-default.conf; then
    echo "    aviso: ya hay un default_server (p. ej. /etc/nginx/sites-enabled/default): no se instala"
    echo "    blacksmith-default.conf. Para que un Host desconocido no reciba ese sitio, desactívelo"
    echo "    (rm /etc/nginx/sites-enabled/default) y vuelva a ejecutar este script."
else
    install -m 0644 "$HERE/conf.d/blacksmith-default.conf" /etc/nginx/conf.d/blacksmith-default.conf
fi

echo "==> Plantillas (/etc/blacksmith/nginx/templates)"
install -d -m 0755 /etc/blacksmith /etc/blacksmith/nginx /etc/blacksmith/nginx/templates
for template in "$HERE"/templates/*.conf; do
    target=/etc/blacksmith/nginx/templates/$(basename "$template")
    if [ -e "$target" ]; then
        cmp -s "$template" "$target" || echo "    se conserva su versión de $(basename "$template")"
    else
        install -m 0644 "$template" "$target"
    fi
done

echo "==> Unidades privilegiadas"
install -d -m 0755 /usr/local/lib/blacksmith
install -m 0755 "$HERE/check-sites.sh" "$HERE/certbot.sh" /usr/local/lib/blacksmith/
sed -e "s|@SITES_DIR@|$SITES_DIR|" -e "s|@APPS_ROOT@|$APPS_ROOT|" "$HERE/systemd/blacksmith-nginx.service" \
    >/etc/systemd/system/blacksmith-nginx.service
sed "s|@CERTS_DIR@|$CERTS_DIR|" "$HERE/systemd/blacksmith-certbot@.service" >/etc/systemd/system/blacksmith-certbot@.service
chmod 0644 /etc/systemd/system/blacksmith-nginx.service /etc/systemd/system/blacksmith-certbot@.service
systemctl daemon-reload

echo "==> polkit"
if [ "$MODE" = local ]; then
    sed "s|@USER@|$AGENT_USER|g" "$HERE/polkit/10-blacksmith-nginx-local.rules.in" \
        >/etc/polkit-1/rules.d/10-blacksmith-nginx-local.rules
    chmod 0644 /etc/polkit-1/rules.d/10-blacksmith-nginx-local.rules
    # nginx (www-data) solo necesita atravesar el directorio personal hasta las carpetas públicas
    # de los servicios: permiso de paso (x), sin listar ni leer nada más.
    if command -v setfacl >/dev/null; then
        for dir in "$HOME_DIR" "$HOME_DIR/.local" "$STATE"; do
            setfacl -m u:www-data:x "$dir"
        done
    else
        echo "    aviso: falta setfacl (apt install acl): nginx no podrá servir sitios estáticos locales"
    fi
else
    install -o root -g root -m 0644 "$HERE/../polkit/10-blacksmith.rules" /etc/polkit-1/rules.d/
fi

command -v certbot >/dev/null || echo "    aviso: certbot no está instalado (apt install certbot): no habrá HTTPS"

echo "==> Validando y recargando nginx"
TARGET=/etc/nginx/conf.d/blacksmith.conf
BACKUP=$(mktemp)
had_previous=no
if [ -e "$TARGET" ]; then cp -p "$TARGET" "$BACKUP" && had_previous=yes; fi
install -m 0644 "$COMMON" "$TARGET"
rm -f "$COMMON"
if ! nginx -t; then
    if [ "$had_previous" = yes ]; then install -m 0644 "$BACKUP" "$TARGET"; else rm -f "$TARGET"; fi
    rm -f "$BACKUP"
    echo "nginx rechazó la configuración: se restauró la anterior" >&2
    exit 1
fi
rm -f "$BACKUP"
systemctl enable --quiet nginx
systemctl reload-or-restart nginx
echo "Listo."
