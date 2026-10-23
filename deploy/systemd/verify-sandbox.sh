#!/bin/sh
# Verifica el sandbox de los servicios con el systemd real del servidor. Ejecutar como root,
# después de instalar las plantillas:
#   sudo sh deploy/systemd/verify-sandbox.sh
#
# Lanza un servicio transitorio con las MISMAS propiedades que /etc/systemd/system/blacksmith@.service
# (instancia «verificacion») y desde dentro intenta salir de su directorio. No toca servicios
# reales: crea un vecino de prueba (servicio y clon) y lo borra al terminar.
set -eu

[ "$(id -u)" -eq 0 ] || { echo "ejecútelo como root" >&2; exit 1; }
TEMPLATE=/etc/systemd/system/blacksmith@.service
[ -f "$TEMPLATE" ] || { echo "falta $TEMPLATE (instale las plantillas primero)" >&2; exit 1; }
for user in blacksmith blacksmith-apps; do
    getent passwd "$user" >/dev/null || { echo "falta el usuario $user (ver README)" >&2; exit 1; }
done

NAME=verificacion
UNIT="blacksmith-sandbox-$NAME"
OWN=/var/lib/blacksmith-apps/$NAME
NEIGHBOUR=/var/lib/blacksmith-apps/$NAME-vecino
NEIGHBOUR_CLONE=/var/lib/blacksmith-fetch/$NAME-vecino
# El agente, si está en marcha (0 si no): un servicio no debe poder mandarle señales.
AGENT_PID=$(systemctl show -p MainPID --value blacksmith.service 2>/dev/null || echo 0)

cleanup() {
    systemctl stop "$UNIT" 2>/dev/null || true
    systemctl reset-failed "$UNIT" 2>/dev/null || true
    rm -rf "$OWN" "$NEIGHBOUR" "$NEIGHBOUR_CLONE"
}
trap cleanup EXIT

mkdir -p "$NEIGHBOUR" "$NEIGHBOUR_CLONE"
echo secreto >"$NEIGHBOUR/secreto"
echo codigo >"$NEIGHBOUR_CLONE/codigo"
chown -R blacksmith-apps:blacksmith-apps "$NEIGHBOUR"
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
check "no corre como el agente"                    "test \"\$(id -un)\" = blacksmith-apps"
check "escribe en su directorio"                  "touch /var/lib/blacksmith-apps/'"$NAME"'/escrito"
check "escribe en su /tmp privado"                "touch /tmp/escrito"
check "no escribe fuera (cd ..)"                  "! (cd /var/lib/blacksmith-apps/'"$NAME"'/.. && touch fuera)"
check "no ve otros servicios"                     "test ! -e '"$NEIGHBOUR"'/secreto"
check "solo ve su directorio en /var/lib/blacksmith-apps" "test \"\$(ls /var/lib/blacksmith-apps)\" = '"$NAME"'"
check "no ve los clones (código de otros)"        "test ! -e '"$NEIGHBOUR_CLONE"'/codigo"
check "no ve las variables (/etc/blacksmith)"     "! ls /etc/blacksmith"
check "no ve los datos del agente (token)"        "! ls /var/lib/blacksmith"
check "no ve el bus del sistema (D-Bus)"          "test ! -S /run/dbus/system_bus_socket"
check "no para otros servicios"                   "! systemctl --no-ask-password stop blacksmith@'"$NAME"'-vecino.service"
check "no manda señales al agente"                "test '"$AGENT_PID"' = 0 || ! kill -0 '"$AGENT_PID"'"
check "no ve /home"                               "test -z \"\$(ls -A /home 2>/dev/null)\""
check "no escribe en /etc"                        "! touch /etc/blacksmith-escrito"
check "no escribe en /usr/local"                  "! touch /usr/local/blacksmith-escrito"
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
