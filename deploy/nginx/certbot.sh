#!/bin/sh
# Pide (o renueva) el certificado <nombre> con certbot. Lo ejecuta blacksmith-certbot@<nombre>, como
# root, con BLACKSMITH_DOMAINS (separados por comas; el primero es <nombre>) y BLACKSMITH_EMAIL
# (opcional) del archivo que deja el agente.
#
# Ese archivo lo escribe un usuario sin privilegios: aquí se vuelven a validar los valores y los
# argumentos de certbot se construyen sin pasar nada suyo como opción. Así, un agente comprometido
# no puede añadir `--pre-hook`, `--deploy-hook` ni similares, que ejecutarían comandos como root.
set -euf

DOMAIN_RE='([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?'
EMAIL_RE='[A-Za-z0-9._%+][A-Za-z0-9._%+-]{0,63}@'"$DOMAIN_RE"

valid() {
    [ "${#2}" -le 253 ] && printf '%s\n' "$2" | grep -Eqx "$1"
}

[ $# -eq 1 ] && valid "$DOMAIN_RE" "$1" || { echo "nombre de certificado inválido" >&2; exit 2; }
NAME=$1

set -- certonly --webroot --webroot-path /var/lib/blacksmith-acme --non-interactive --agree-tos \
    --keep-until-expiring --cert-name "$NAME"

first=
old_ifs=$IFS
IFS=,
for domain in ${BLACKSMITH_DOMAINS:-}; do
    valid "$DOMAIN_RE" "$domain" || { echo "dominio inválido: $domain" >&2; exit 2; }
    [ -n "$first" ] || first=$domain
    set -- "$@" --domain "$domain"
done
IFS=$old_ifs
[ "$first" = "$NAME" ] || { echo "el primer dominio debe ser $NAME" >&2; exit 2; }

email=${BLACKSMITH_EMAIL:-}
if [ -n "$email" ]; then
    valid "$EMAIL_RE" "$email" || { echo "correo inválido" >&2; exit 2; }
    set -- "$@" --email "$email"
else
    set -- "$@" --register-unsafely-without-email
fi

exec /usr/bin/certbot "$@" --deploy-hook "systemctl reload nginx"
