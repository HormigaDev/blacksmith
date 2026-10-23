#!/bin/sh
# Instala la integración de nginx de BBEL Blacksmith. Ejecutar como root desde la raíz del
# repositorio:
#   sh deploy/nginx/install.sh            servidor: el agente corre como `blacksmith`
#   sh deploy/nginx/install.sh --local    modo local: el agente corre como su usuario (con
#                                         privilegios de root obtenidos desde ese usuario)
#
# Qué hace (es idempotente; repítalo tras actualizar Blacksmith):
#   - /etc/nginx/conf.d/bbel.conf: ajustes comunes e `include` de los sitios que genera el agente.
#   - /etc/nginx/conf.d/bbel-default.conf: cierra las peticiones a hosts desconocidos (solo si no
#     hay ya otro `default_server`; ver el aviso).
#   - /etc/nginx/snippets/bbel/: TLS, cabeceras de seguridad, proxy, WebSocket, estáticos, ACME.
#   - /etc/bbel/nginx/templates/: plantillas por tipo de proyecto. Las que ya existan NO se
#     sobrescriben: son suyas para editarlas.
#   - /var/lib/bbel-acme: desafíos de Let's Encrypt.
#   - bbel-nginx.service y bbel-certbot@.service: lo único que corre como root por cuenta del
#     agente (validar/recargar nginx y pedir certificados), y la regla de polkit que se lo permite.
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
else
    AGENT_USER=blacksmith
    STATE=/var/lib
fi
SITES_DIR=$STATE/blacksmith/nginx/sites
CERTS_DIR=$STATE/blacksmith/nginx/certs

echo "==> Modo $MODE (agente: $AGENT_USER)"

# Directorios del agente (también los crea él al arrancar).
install -d -m 0700 -o "$AGENT_USER" "$STATE/blacksmith" "$STATE/blacksmith/nginx" "$SITES_DIR" "$CERTS_DIR"
install -d -m 0755 /var/lib/bbel-acme

echo "==> Snippets y ajustes comunes"
install -d -m 0755 /etc/nginx/snippets/bbel
install -m 0644 "$HERE"/snippets/bbel/*.conf /etc/nginx/snippets/bbel/
sed "s|@SITES_DIR@|$SITES_DIR|" "$HERE/conf.d/bbel.conf" >/etc/nginx/conf.d/bbel.conf
chmod 0644 /etc/nginx/conf.d/bbel.conf

if grep -rqsE 'listen[^;]*default_server' /etc/nginx/sites-enabled /etc/nginx/conf.d --exclude=bbel-default.conf; then
    echo "    aviso: ya hay un default_server (p. ej. /etc/nginx/sites-enabled/default): no se instala"
    echo "    bbel-default.conf. Para que un Host desconocido no reciba ese sitio, desactívelo"
    echo "    (rm /etc/nginx/sites-enabled/default) y vuelva a ejecutar este script."
else
    install -m 0644 "$HERE/conf.d/bbel-default.conf" /etc/nginx/conf.d/bbel-default.conf
fi

echo "==> Plantillas (/etc/bbel/nginx/templates)"
install -d -m 0755 /etc/bbel /etc/bbel/nginx /etc/bbel/nginx/templates
for template in "$HERE"/templates/*.conf; do
    target=/etc/bbel/nginx/templates/$(basename "$template")
    if [ -e "$target" ]; then
        cmp -s "$template" "$target" || echo "    se conserva su versión de $(basename "$template")"
    else
        install -m 0644 "$template" "$target"
    fi
done

echo "==> Unidades privilegiadas"
install -m 0644 "$HERE/systemd/bbel-nginx.service" /etc/systemd/system/bbel-nginx.service
sed "s|@CERTS_DIR@|$CERTS_DIR|" "$HERE/systemd/bbel-certbot@.service" >/etc/systemd/system/bbel-certbot@.service
chmod 0644 /etc/systemd/system/bbel-certbot@.service
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
nginx -t
systemctl enable --quiet nginx
systemctl reload-or-restart nginx
echo "Listo."
