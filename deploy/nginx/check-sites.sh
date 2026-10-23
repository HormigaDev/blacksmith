#!/bin/sh
# Comprueba, como root y antes de `nginx -t`, los sitios que escribe el agente.
#   check-sites.sh <directorio de sitios> <raíz de los servicios>
#
# Los escribe un usuario sin privilegios, pero los carga el maestro de nginx, que corre como root:
# un agente comprometido podría escribir `access_log /etc/cron.d/x` o `include /etc/shadow` y
# convertirse en root. La API del agente ya valida todo; esto es la defensa de quien tiene el
# privilegio, que no se fía del archivo:
#   - Las directivas que leen o escriben archivos solo pueden usar rutas conocidas: la carpeta
#     pública de los servicios, el webroot de ACME, los logs de nginx, los certificados de
#     Let's Encrypt y los snippets de Blacksmith. Cualquier otra directiva con rutas se rechaza.
#   - Las que ejecutan código (perl, njs, lua) se rechazan siempre, y ningún valor puede apuntar a
#     un socket Unix (`proxy_pass http://unix:/var/run/docker.sock:` expondría a internet un
#     servicio local con privilegios).
# El análisis respeta comillas, escapes y comentarios como nginx: un `#` dentro de un valor
# entrecomillado no puede ocultar una directiva.
set -eu

[ $# -eq 2 ] || { echo "uso: check-sites.sh <sitios> <raíz de los servicios>" >&2; exit 2; }
SITES_DIR=$1
APPS_ROOT=$2
[ -d "$SITES_DIR" ] || exit 0

status=0
for file in "$SITES_DIR"/*.conf; do
    [ -e "$file" ] || continue
    LC_ALL=C awk -v apps="$APPS_ROOT/" -v file="$file" '
        function fail(directive, reason) {
            printf "%s: «%s» %s\n", file, directive, reason > "/dev/stderr"
            bad = 1
        }
        function under(path, prefix) {
            return index(path, prefix) == 1 && index(path, "..") == 0
        }
        function check(    n, word, directive, path, k) {
            n = split(statement, word, "\034")
            statement = ""
            if (n < 1 || word[1] == "") return
            directive = word[1]
            path = (n >= 2) ? word[2] : ""
            for (k = 2; k <= n; k++)
                if (index(word[k], "unix:") > 0) fail(directive, "apunta a un socket Unix")
            if (directive ~ /^(perl|js_|lua|load_module|env$)/ || directive ~ /_by_lua/) {
                fail(directive, "no está permitida")
            } else if (directive == "include") {
                if (!under(path, "snippets/blacksmith/") && !under(path, "/etc/nginx/snippets/blacksmith/"))
                    fail(directive " " path, "fuera de los snippets de Blacksmith")
            } else if (directive == "root" || directive == "alias") {
                if (!under(path, apps) && path != "/var/lib/blacksmith-acme" && !under(path, "/var/lib/blacksmith-acme/"))
                    fail(directive " " path, "fuera de los servicios")
            } else if (directive == "access_log" || directive == "error_log") {
                if (path != "off" && !under(path, "/var/log/nginx/"))
                    fail(directive " " path, "fuera de /var/log/nginx")
            } else if (directive ~ /_(certificate|certificate_key|trusted_certificate)$/) {
                if (!under(path, "/etc/letsencrypt/live/"))
                    fail(directive " " path, "fuera de /etc/letsencrypt/live")
            } else if (directive ~ /(_temp_path|_cache_path|_file|^ssl_(dhparam|crl|client_certificate|stapling_file|session_ticket_key|password_file))$/) {
                fail(directive, "usa archivos y no está permitida")
            }
        }
        # Palabra en curso: se añade a la sentencia al terminarla.
        function flush() {
            if (have) statement = (statement == "" ? token : statement "\034" token)
            token = ""; have = 0
        }
        {
            line = $0
            for (i = 1; i <= length(line); i++) {
                c = substr(line, i, 1)
                if (quote != "") {
                    if (c == "\\") { token = token substr(line, i + 1, 1); i++; continue }
                    if (c == quote) { quote = ""; continue }
                    token = token c; continue
                }
                if (c == "#") break
                if (c == "\"" || c == "\047") { quote = c; have = 1; continue }
                if (c == " " || c == "\t") { flush(); continue }
                if (c == ";" || c == "{" || c == "}") { flush(); check(); continue }
                token = token c; have = 1
            }
            if (quote == "") flush()
            else token = token "\n"
        }
        END {
            flush(); check()
            if (quote != "") fail("(fin del archivo)", "comillas sin cerrar")
            exit bad
        }
    ' "$file" || status=1
done
exit $status
