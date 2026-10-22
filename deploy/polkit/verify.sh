#!/bin/sh
# Verifica la regla de polkit (10-blacksmith.rules) con el systemd real. Ejecutar como root:
#   sudo sh deploy/polkit/verify.sh
#
# No toca servicios existentes: crea dos unidades de prueba en /run/systemd/system (desaparecen
# al reiniciar) y las borra al terminar. Si el usuario `blacksmith` no existe, lo crea y lo
# elimina al final.
set -eu

[ "$(id -u)" -eq 0 ] || { echo "ejecútelo como root" >&2; exit 1; }
[ -f /etc/polkit-1/rules.d/10-blacksmith.rules ] || {
    echo "falta /etc/polkit-1/rules.d/10-blacksmith.rules" >&2
    exit 1
}

UNITS=/run/systemd/system
MANAGED=bbel-verificacion.service
FOREIGN=blacksmith-verificacion-ajena.service
created_user=no

cleanup() {
    systemctl stop "$MANAGED" "$FOREIGN" 2>/dev/null || true
    rm -f "$UNITS/$MANAGED" "$UNITS/$FOREIGN"
    systemctl daemon-reload
    systemctl reset-failed "$MANAGED" "$FOREIGN" 2>/dev/null || true
    if [ "$created_user" = yes ]; then userdel blacksmith; fi
}
trap cleanup EXIT

if ! getent passwd blacksmith >/dev/null; then
    useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin blacksmith
    created_user=yes
fi

for unit in "$MANAGED" "$FOREIGN"; do
    printf '[Service]\nExecStart=/bin/sleep infinity\nDynamicUser=yes\n' >"$UNITS/$unit"
done
systemctl daemon-reload

failures=0
# expect <permitido|denegado> <verbo> <unidad>
expect() {
    if output=$(runuser -u blacksmith -- env LC_ALL=C systemctl --no-ask-password "$2" -- "$3" 2>&1); then
        got=permitido
    else
        got=denegado
    fi
    if [ "$got" = "$1" ]; then
        printf 'ok     %-9s %-8s %s\n' "$1" "$2" "$3"
    else
        printf 'FALLA  %-9s %-8s %s (obtenido: %s)\n       %s\n' "$1" "$2" "$3" "$got" "$output"
        failures=$((failures + 1))
    fi
}

expect permitido start "$MANAGED"
expect permitido restart "$MANAGED"
expect permitido stop "$MANAGED"
expect denegado enable "$MANAGED"
expect denegado mask "$MANAGED"
expect denegado start "$FOREIGN"
expect denegado stop "$FOREIGN"
expect denegado restart blacksmith.service

# Mensaje exacto de una denegación (el agente lo clasifica como PermissionDenied).
runuser -u blacksmith -- env LC_ALL=C systemctl --no-ask-password start -- "$FOREIGN" 2>&1 | head -1 || true

if [ "$failures" -eq 0 ]; then echo "regla correcta"; else echo "$failures comprobación(es) fallida(s)"; exit 1; fi
