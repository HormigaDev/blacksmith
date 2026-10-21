/**
 * Mensajes en español: idioma BASE y fuente de verdad de las claves.
 * - Toda clave existe aquí primero; los demás idiomas solo traducen claves de este archivo.
 * - Parámetros con `{nombre}`. Plurales: objeto con `one` / `other` (y `zero`, `two`,
 *   `few`, `many` si el idioma los usa), siempre con `{count}`.
 * - Agrupar por pantalla o área funcional; `common` solo para lo verdaderamente compartido.
 * Reglas completas: client/docs/i18n.md
 */
export default {
    common: {
        system: 'Sistema',
        cancel: 'Cancelar',
        confirm: 'Confirmar',
        close: 'Cerrar',
        loading: 'Cargando…',
        required: 'Obligatorio',
    },
    theme: {
        label: 'Tema',
        names: {
            light: 'Claro',
            dark: 'Oscuro',
        },
    },
    locale: {
        label: 'Idioma',
    },
    components: {
        input: {
            increment: 'Aumentar',
            decrement: 'Disminuir',
            showPassword: 'Mostrar contraseña',
            hidePassword: 'Ocultar contraseña',
        },
        textarea: {
            counter: '{count} / {max}',
        },
        select: {
            placeholder: 'Selecciona una opción',
        },
        datePicker: {
            open: 'Abrir calendario',
            previousMonth: 'Mes anterior',
            nextMonth: 'Mes siguiente',
        },
        filePicker: {
            dropFile: 'Arrastra un archivo aquí',
            dropFiles: 'Arrastra archivos aquí',
            dropDirectory: 'Arrastra una carpeta aquí',
            dropActive: 'Suelta para añadir',
            browseFile: 'Seleccionar archivo',
            browseFiles: 'Seleccionar archivos',
            browseDirectory: 'Seleccionar carpeta',
            formats: 'Formatos admitidos: {formats}',
            remove: 'Quitar {name}',
            rejected: {
                one: '{count} archivo no tiene un formato admitido',
                other: '{count} archivos no tienen un formato admitido',
            },
        },
        toast: {
            region: 'Notificaciones',
            dismiss: 'Cerrar notificación',
        },
    },
    errors: {
        network: 'No se pudo conectar con el agente.',
        timeout: 'El agente tardó demasiado en responder.',
        unexpected: 'Ocurrió un error inesperado.',
        agent: {
            BAD_REQUEST: 'La solicitud no es válida.',
            INVALID_JSON: 'Los datos enviados no tienen el formato esperado.',
            UNSUPPORTED_MEDIA_TYPE: 'El formato de los datos no es compatible.',
            PAYLOAD_TOO_LARGE: 'Los datos enviados son demasiado grandes.',
            UNAUTHORIZED: 'Se requiere autenticación.',
            FORBIDDEN: 'El agente rechazó la solicitud.',
            NOT_FOUND: 'No se encontró el recurso.',
            METHOD_NOT_ALLOWED: 'Operación no permitida.',
            CONFLICT: 'La operación entra en conflicto con el estado actual.',
            REQUEST_TIMEOUT: 'El agente tardó demasiado en procesar la solicitud.',
            OVERLOADED: 'El agente está saturado. Inténtalo de nuevo en unos segundos.',
            UNAVAILABLE: 'El servicio no está disponible en este momento.',
            INTERNAL: 'El agente encontró un error interno.',
            UNKNOWN: 'El agente devolvió un error desconocido.',
        },
        tunnel: {
            INVALID_TARGET: 'Revisa el host, el puerto y el usuario.',
            KEY_NOT_FOUND:
                'No se encontró la llave. Sin archivo elegido, se buscan id_ed25519, id_ecdsa e id_rsa en ~/.ssh.',
            KEY_FILE_UNREADABLE: 'No se pudo leer el archivo de la llave. Revisa sus permisos.',
            KEY_TOO_LARGE: 'El archivo es demasiado grande para ser una llave privada.',
            KEY_IS_PUBLIC: 'Esa es la llave pública (.pub). Hace falta la privada.',
            KEY_INVALID: 'El contenido no es una llave privada SSH válida.',
            KEY_UNSUPPORTED: 'Ese tipo de llave no está soportado. Usa ed25519, ECDSA o RSA.',
            PASSPHRASE_REQUIRED: 'La llave está protegida. Escribe su passphrase.',
            PASSPHRASE_INVALID: 'La passphrase no es correcta.',
            CREDENTIALS_MISSING: 'No hay ninguna llave guardada para este servidor.',
            KEYRING_UNAVAILABLE:
                'El llavero del sistema no está disponible. En Linux hace falta GNOME Keyring o KWallet en marcha.',
            VAULT_UNREADABLE: 'Las credenciales guardadas no se pueden descifrar. Vuelve a añadir la llave.',
            HOST_UNREACHABLE: 'No se pudo contactar con el servidor. Revisa el host, el puerto y tu conexión.',
            CONNECTION_TIMEOUT: 'El servidor no respondió a tiempo.',
            HOST_KEY_UNKNOWN: 'Es la primera vez que te conectas a este servidor. Confirma su huella.',
            HOST_KEY_CHANGED:
                'La llave del servidor cambió desde la última conexión. Podría tratarse de un ataque: no se ha conectado.',
            HOST_KEY_NOT_PENDING: 'La huella ya no coincide con la presentada. Vuelve a conectar.',
            AUTH_REJECTED: 'El servidor rechazó la llave. Comprueba el usuario y que la llave esté autorizada.',
            AGENT_UNREACHABLE: 'La conexión SSH funciona, pero el agente de Blacksmith no responde en el servidor.',
            PORT_IN_USE: 'El puerto local {port} está en uso.',
            INTERNAL: 'Ocurrió un error inesperado al conectar.',
        },
    },
    servers: {
        title: 'Servidores',
        empty: {
            title: 'Sin servidores todavía',
            description: 'Añade un VPS con su llave SSH. Se conectará por un túnel, sin exponer puertos.',
        },
        add: {
            open: 'Añadir servidor',
            title: 'Añadir servidor',
            description:
                'El servidor queda guardado aunque el agente aún no esté instalado; podrás conectar cuando lo esté.',
            submit: 'Guardar y conectar',
        },
        status: {
            disconnected: 'Desconectado',
            connecting: 'Conectando…',
            connected: 'Conectado',
            agentMissing: 'Agente no configurado',
            error: 'Error',
        },
        card: {
            connect: 'Conectar',
            retry: 'Reintentar',
            agentMissingHint:
                'SSH funciona, pero el agente de Blacksmith no responde en el servidor. Instálalo o arráncalo y vuelve a intentarlo.',
        },
        remove: {
            action: 'Quitar servidor',
            label: 'Quitar {address}',
            title: '¿Quitar {address}?',
            description:
                'Se cerrará su conexión y se borrará la llave guardada en este equipo. El servidor no se modifica.',
            confirm: 'Quitar',
        },
        passphrase: {
            title: 'Passphrase de {host}',
            description: 'La llave de este servidor está protegida. Escríbela para conectar.',
            invalid: 'La passphrase no es correcta.',
        },
        connect: {
            title: 'Conectar servidor',
            description:
                'La conexión va por un túnel SSH con tu llave; ningún puerto de administración queda expuesto.',
            open: 'Conectar servidor',
            host: 'Host',
            hostHint: 'Dominio o IP del VPS',
            hostPlaceholder: 'vps.ejemplo.com',
            usernamePlaceholder: 'deploy',
            keyContentPlaceholder: '-----BEGIN OPENSSH PRIVATE KEY-----',
            port: 'Puerto SSH',
            username: 'Usuario',
            keySource: 'Llave privada',
            sources: {
                file: 'Archivo',
                inline: 'Pegar',
                default: 'Predeterminada',
            },
            keyFile: 'Archivo de la llave',
            keyContent: 'Contenido de la llave',
            keyContentHint: 'Pega la llave privada completa, desde «-----BEGIN» hasta «-----END».',
            defaultKeyHint: 'Se usará la primera de ~/.ssh/id_ed25519, id_ecdsa o id_rsa.',
            storageHint:
                'La llave se guarda cifrada en este equipo; la clave de cifrado vive en el llavero del sistema.',
            passphrase: 'Passphrase de la llave',
            rememberPassphrase: 'Recordar la passphrase en este equipo',
            submit: 'Conectar',
            required: 'Campo obligatorio',
            connected: 'Conectado a {host}',
            connectedDetail: 'Túnel activo en 127.0.0.1:{port}',
            disconnect: 'Desconectar',
            disconnected: 'Desconectado de {host}',
            lost: 'Se perdió la conexión con {host}',
            trust: {
                title: '¿Confiar en este servidor?',
                description:
                    'Es la primera vez que te conectas a {host}. Comprueba que la huella coincide con la del servidor antes de continuar.',
                fingerprint: 'Huella ({algorithm})',
                verifyHint: 'En el servidor: ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub',
                confirm: 'Confiar y conectar',
            },
            changed: {
                forget: 'Olvidar la llave registrada',
                title: '¿Olvidar la llave registrada de {host}?',
                description:
                    'Hazlo solo si sabes por qué cambió (p. ej. reinstalaste el servidor). Al volver a conectar tendrás que confirmar la nueva huella.',
                confirm: 'Olvidar llave',
            },
        },
    },
    dashboard: {
        title: 'Panel',
        empty: {
            title: 'Sin servidores todavía',
            description: 'Aquí aparecerán los servidores y servicios que administres.',
        },
    },
} as const;
