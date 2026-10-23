#!/bin/sh
# Verifica el sandbox de los servicios con el systemd real del servidor. Ejecutar como root,
# después de instalar las plantillas:
#   sudo sh deploy/systemd/verify-sandbox.sh
#
# Lanza un servicio transitorio con las MISMAS propiedades que /etc/systemd/system/bbel@.service
# (instancia «verificacion») y desde dentro intenta salir de su directorio. No toca servicios
# reales: crea un vecino de prueba en /var/lib/bbel y lo borra al terminar.
set -eu

[ "$(id -u)" -eq 0 ] || { echo "ejecútelo como root" >&2; exit 1; }
TEMPLATE=/etc/systemd/system/bbel@.service
[ -f "$TEMPLATE" ] || { echo "falta $TEMPLATE (instale las plantillas primero)" >&2; exit 1; }
getent passwd bbel >/dev/null || { echo "falta el usuario bbel (ver README)" >&2; exit 1; }

NAME=verificacion
UNIT="bbel-sandbox-$NAME"
OWN=/var/lib/bbel/$NAME
NEIGHBOUR=/var/lib/bbel/$NAME-vecino

cleanup() {
    systemctl stop "$UNIT" 2>/dev/null || true
    systemctl reset-failed "$UNIT" 2>/dev/null || true
    rm -rf "$OWN" "$NEIGHBOUR"
}
trap cleanup EXIT

mkdir -p "$NEIGHBOUR"
echo secreto >"$NEIGHBOUR/secreto"
chown -R bbel:bbel "$NEIGHBOUR"
chmod 0700 "$NEIGHBOUR"

# Propiedades de la sección [Service] de la plantilla, con los especificadores resueltos. Se
# omite lo que es propio de un servicio de larga duración o de su versión desplegada.
set --
while IFS= read -r line; do
    case "$line" in
        '' | '#'* | '['*) continue ;;
        Type=* | ExecStart=* | WorkingDirectory=* | Restart=* | RestartSec=* | Description=*) continue ;;
        After=* | Wants=* | Condition*) continue ;;
    esac
    property=$(printf '%s' "$line" | sed -e "s|%i|$NAME|g" -e 's|%S|/var/lib|g' -e 's|%E|/etc|g')
    set -- "$@" -p "$property"
done <<EOF
$(sed -n '/^\[Service\]/,$p' "$TEMPLATE")
EOF

CHECKS='
check() { if eval "$2" >/dev/null 2>&1; then echo "ok     $1"; else echo "FALLA  $1"; fi; }
check "escribe en su directorio"                  "touch /var/lib/bbel/'"$NAME"'/escrito"
check "escribe en su /tmp privado"                "touch /tmp/escrito"
check "no escribe fuera (cd ..)"                  "! (cd /var/lib/bbel/'"$NAME"'/.. && touch fuera)"
check "no ve otros servicios"                     "test ! -e '"$NEIGHBOUR"'/secreto"
check "solo ve su directorio en /var/lib/bbel"    "test \"\$(ls /var/lib/bbel)\" = '"$NAME"'"
check "no ve las variables (/etc/bbel)"           "! ls /etc/bbel"
check "no ve los datos del agente (token)"        "! ls /var/lib/blacksmith"
check "no ve /home"                               "test -z \"\$(ls -A /home 2>/dev/null)\""
check "no escribe en /etc"                        "! touch /etc/bbel-escrito"
check "no escribe en /usr/local"                  "! touch /usr/local/bbel-escrito"
check "no ve procesos ajenos"                     "test \"\$(ls /proc | grep -c \"^[0-9]\")\" -lt 10"
check "no ve credenciales de otras unidades"      "! ls /run/credentials"
'

echo "Servicio de prueba $UNIT con las propiedades de $TEMPLATE:"
output=$(systemd-run --quiet --wait --pipe --collect --unit="$UNIT" "$@" /bin/sh -c "$CHECKS")
printf '%s\n' "$output"
failures=$(printf '%s\n' "$output" | grep -c '^FALLA' || true)
if [ "$failures" -gt 0 ]; then
    echo "$failures comprobación(es) fallida(s)"
    exit 1
fi
echo "Sandbox correcto."
