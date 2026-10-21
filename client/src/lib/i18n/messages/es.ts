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
    },
    dashboard: {
        title: 'Panel',
        empty: {
            title: 'Sin servidores todavía',
            description: 'Aquí aparecerán los servidores y servicios que administres.',
        },
    },
} as const;
