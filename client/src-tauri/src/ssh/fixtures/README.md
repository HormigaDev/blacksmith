# Llaves de TEST

Generadas con `ssh-keygen` solo para los tests del túnel (`src/ssh`). No protegen nada, son
públicas y **nunca** deben usarse en un servidor real.

- `client_*`: llaves del cliente (ed25519, ed25519 cifrada con la passphrase
  `correct horse battery staple`, RSA OpenSSH, RSA PKCS#1 PEM y ECDSA P-256).
- `intruder_ed25519`: llave de cliente no autorizada.
- `host_ed25519` / `host_impostor_ed25519`: llave del servidor de test y la de un impostor
  (cambio de huella).
