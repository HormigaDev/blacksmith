#!/bin/sh
# Publica los webhooks de GitHub del agente en <dominio> (despliegue automático al hacer push).
# Ejecutar como root desde la raíz del repositorio, DESPUÉS de deploy/nginx/install.sh:
#   sh deploy/nginx/install-webhooks.sh hooks.ejemplo.com [correo-para-letsencrypt]
#
# Antes: el dominio debe apuntar ya a este servidor (registro A/AAAA) y los puertos 80 y 443 estar
# abiertos. Es idempotente: repítalo tras actualizar Blacksmith.
#
# Qué hace:
#   1. Crea el secreto del webhook (/etc/blacksmith/credentials/github-webhook-secret, root 0600) si
#      no existe, y se lo da al agente como credencial de systemd con un drop-in
#      (/etc/systemd/system/blacksmith.service.d/webhooks.conf). Reinicia el agente.
#   2. Pide el certificado de Let's Encrypt del dominio si no lo hay. Primero publica solo el
#      desafío ACME por HTTP: la configuración completa apunta al certificado y nginx no la
#      aceptaría sin él.
#   3. Instala /etc/nginx/conf.d/blacksmith-webhooks.conf (solo `POST /api/webhooks/github`), lo
#      valida con `nginx -t` (si falla, deja lo anterior) y recarga nginx.
#   4. Muestra la URL y el secreto que hay que poner en GitHub.
set -eu

[ "$(id -u)" -eq 0 ] || { echo "ejecútelo como root" >&2; exit 1; }
[ $# -ge 1 ] && [ $# -le 2 ] || { echo "uso: install-webhooks.sh <dominio> [correo]" >&2; exit 2; }
DOMAIN=$1
EMAIL=${2:-}
HERE=$(cd "$(dirname "$0")" && pwd)

printf '%s\n' "$DOMAIN" | grep -Eqx '([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]*' \
    || { echo "dominio inválido (en minúsculas, p. ej. hooks.ejemplo.com): $DOMAIN" >&2; exit 2; }
if [ -n "$EMAIL" ]; then
    printf '%s\n' "$EMAIL" | grep -Eqx '[A-Za-z0-9._%+][A-Za-z0-9._%+-]*@[A-Za-z0-9.-]+\.[A-Za-z]+' \
        || { echo "correo inválido: $EMAIL" >&2; exit 2; }
fi
[ -f /etc/nginx/snippets/blacksmith/acme.conf ] \
    || { echo "falta la integración de nginx: ejecute antes sh deploy/nginx/install.sh" >&2; exit 1; }
[ -f /etc/systemd/system/blacksmith.service ] \
    || { echo "falta el agente: instale antes deploy/blacksmith.service (ver README)" >&2; exit 1; }
command -v certbot >/dev/null || { echo "falta certbot (apt install certbot)" >&2; exit 1; }

CONF=/etc/nginx/conf.d/blacksmith-webhooks.conf
SECRET=/etc/blacksmith/credentials/github-webhook-secret
DROPIN=/etc/systemd/system/blacksmith.service.d/webhooks.conf
CERT=/etc/letsencrypt/live/$DOMAIN/fullchain.pem

echo "==> Secreto del webhook"
install -d -o root -g root -m 0700 /etc/blacksmith/credentials
if [ ! -s "$SECRET" ]; then
    (umask 077 && head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' >"$SECRET")
fi
chown root:root "$SECRET"
chmod 0600 "$SECRET"
install -d -m 0755 /etc/systemd/system/blacksmith.service.d
cat >"$DROPIN" <<'EOF'
# Generado por deploy/nginx/install-webhooks.sh: el secreto del webhook como credencial de systemd
# (tmpfs privado, fuera del entorno; no lo heredan los procesos hijos).
[Service]
LoadCredential=github-webhook-secret:/etc/blacksmith/credentials/github-webhook-secret
Environment=BLACKSMITH_GITHUB_WEBHOOK_SECRET_FILE=%d/github-webhook-secret
EOF
chmod 0644 "$DROPIN"
systemctl daemon-reload
systemctl restart blacksmith.service

reload_nginx() {
    nginx -t && systemctl reload nginx
}

if [ ! -f "$CERT" ]; then
    echo "==> Certificado de $DOMAIN"
    # Solo el desafío ACME por HTTP, hasta tener el certificado.
    cat >"$CONF" <<EOF
server {
    listen 80;
    listen [::]:80;
    server_name $DOMAIN;
    include snippets/blacksmith/acme.conf;
    location / {
        return 444;
    }
}
EOF
    reload_nginx
    if [ -n "$EMAIL" ]; then account="--email $EMAIL"; else account=--register-unsafely-without-email; fi
    # shellcheck disable=SC2086 # $account son dos palabras a propósito.
    if ! certbot certonly --webroot --webroot-path /var/lib/blacksmith-acme --non-interactive \
        --agree-tos $account --cert-name "$DOMAIN" --domain "$DOMAIN" \
        --deploy-hook "systemctl reload nginx"; then
        rm -f "$CONF"
        reload_nginx
        echo "certbot no pudo emitir el certificado: ¿apunta $DOMAIN a este servidor y está abierto el puerto 80?" >&2
        exit 1
    fi
fi

echo "==> nginx ($CONF)"
NEW=$(mktemp)
sed "s/hooks\.example\.com/$DOMAIN/g" "$HERE/webhooks.conf" >"$NEW"
BACKUP=$(mktemp)
had_previous=no
if [ -e "$CONF" ]; then cp -p "$CONF" "$BACKUP" && had_previous=yes; fi
install -m 0644 "$NEW" "$CONF"
rm -f "$NEW"
if ! nginx -t; then
    if [ "$had_previous" = yes ]; then install -m 0644 "$BACKUP" "$CONF"; else rm -f "$CONF"; fi
    rm -f "$BACKUP"
    echo "nginx rechazó la configuración: se restauró la anterior" >&2
    exit 1
fi
rm -f "$BACKUP"
systemctl reload nginx

echo
echo "Listo. En GitHub (Settings > Webhooks > Add webhook, en cada repositorio):"
echo "  Payload URL:  https://$DOMAIN/api/webhooks/github"
echo "  Content type: application/json"
echo "  Secret:       $(cat "$SECRET")"
echo "  Eventos:      Just the push event"
