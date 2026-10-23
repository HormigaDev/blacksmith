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
MANAGED=blacksmith@verificacion.service
DEPLOY=blacksmith-deploy@verificacion.service
CERT=blacksmith-certbot@verificacion.example.service
FOREIGN=blacksmith-verificacion-ajena.service
created_user=no

cleanup() {
    systemctl stop "$MANAGED" "$DEPLOY" "$CERT" "$FOREIGN" 2>/dev/null || true
    # Quita los drop-ins de set-property (/etc/systemd/system.control/).
    systemctl revert "$MANAGED" "$DEPLOY" "$FOREIGN" >/dev/null 2>&1 || true
    rm -f "$UNITS/$MANAGED" "$UNITS/$DEPLOY" "$UNITS/$CERT" "$UNITS/$FOREIGN"
    systemctl daemon-reload
    systemctl reset-failed "$MANAGED" "$DEPLOY" "$CERT" "$FOREIGN" 2>/dev/null || true
    if [ "$created_user" = yes ]; then userdel blacksmith; fi
}
trap cleanup EXIT

if ! getent passwd blacksmith >/dev/null; then
    useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin blacksmith
    created_user=yes
fi

for unit in "$MANAGED" "$DEPLOY" "$CERT" "$FOREIGN"; do
    printf '[Service]\nExecStart=/bin/sleep infinity\nDynamicUser=yes\n' >"$UNITS/$unit"
done
systemctl daemon-reload

failures=0
# expect <permitido|denegado> <verbo> <unidad> [propiedad…]
expect() {
    wanted=$1 verb=$2 unit=$3
    shift 3
    if output=$(runuser -u blacksmith -- env LC_ALL=C systemctl --no-ask-password "$verb" -- "$unit" "$@" 2>&1); then
        got=permitido
    else
        got=denegado
    fi
    if [ "$got" = "$wanted" ]; then
        printf 'ok     %-9s %-12s %s\n' "$wanted" "$verb" "$unit"
    else
        printf 'FALLA  %-9s %-12s %s (obtenido: %s)\n       %s\n' "$wanted" "$verb" "$unit" "$got" "$output"
        failures=$((failures + 1))
    fi
}

expect permitido start "$MANAGED"
expect permitido restart "$MANAGED"
expect permitido stop "$MANAGED"
expect permitido start "$DEPLOY"
expect permitido stop "$DEPLOY"
expect denegado restart "$DEPLOY"
# Límites de recursos: solo de los servicios, nunca de sus despliegues ni de otras unidades.
expect permitido set-property "$MANAGED" MemoryMax=268435456 CPUQuota=50%
expect permitido set-property "$MANAGED" MemoryMax=infinity CPUQuota=
expect denegado set-property "$DEPLOY" MemoryMax=268435456
expect denegado set-property "$FOREIGN" MemoryMax=268435456
expect denegado set-property blacksmith.service MemoryMax=infinity
expect denegado enable "$MANAGED"
expect denegado mask "$MANAGED"
expect denegado start "$FOREIGN"
expect denegado stop "$FOREIGN"
expect denegado restart blacksmith.service
# nginx: solo pedir certificados y aplicar la configuración; nunca parar ni tocar nginx.
expect permitido start "$CERT"
expect denegado stop "$CERT"
expect denegado restart nginx.service
expect denegado stop nginx.service

# Una unidad transitoria con el prefijo permitido llevaría cualquier comando y se ejecutaría
# como root: crearla debe estar denegado aunque el nombre encaje con la regla.
if runuser -u blacksmith -- env LC_ALL=C systemd-run --no-ask-password --quiet \
    --unit=blacksmith@verificacion-transitoria -- /bin/true >/dev/null 2>&1; then
    printf 'FALLA  denegado  %-8s %s\n' "run" "blacksmith@verificacion-transitoria (systemd-run)"
    systemctl stop blacksmith@verificacion-transitoria.service 2>/dev/null || true
    failures=$((failures + 1))
else
    printf 'ok     %-9s %-8s %s\n' "denegado" "run" "blacksmith@verificacion-transitoria (systemd-run)"
fi

# Mensaje exacto de una denegación (el agente lo clasifica como PermissionDenied).
runuser -u blacksmith -- env LC_ALL=C systemctl --no-ask-password start -- "$FOREIGN" 2>&1 | head -1 || true

if [ "$failures" -eq 0 ]; then echo "regla correcta"; else echo "$failures comprobación(es) fallida(s)"; exit 1; fi
