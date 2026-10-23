/**
 * Reglas que también aplica el agente (`agent/src/services/deployment.rs`, `envfile.rs`,
 * `nginx/sites.rs`). El agente es quien decide; esto solo adelanta el error junto al campo en vez
 * de un rechazo genérico al guardar. Si cambia una regla allí, cambia aquí.
 */

const bytes = (text: string) => new TextEncoder().encode(text).length;

/** Caracteres de control (C0, DEL y C1). */
// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;

export const hasControlChars = (text: string): boolean => CONTROL.test(text);

/** Nombre de rama válido para git (`validate_branch`). */
export function validBranch(branch: string): boolean {
    return (
        branch !== '' &&
        bytes(branch) <= 255 &&
        !hasControlChars(branch) &&
        !/[ ~^:?*[\\]/.test(branch) &&
        !/^[-/]/.test(branch) &&
        !/[/.]$/.test(branch) &&
        !branch.includes('..') &&
        !branch.includes('//') &&
        !branch.includes('@{') &&
        branch !== '@' &&
        branch.split('/').every((part) => !part.startsWith('.') && !part.endsWith('.lock'))
    );
}

/** Variables de entorno: límites de `envfile.rs`. */
export const MAX_ENV_VARIABLES = 256;
export const MAX_ENV_KEY_BYTES = 128;
export const MAX_ENV_VALUE_BYTES = 32 * 1024;

export const envKeyTooLong = (key: string): boolean => bytes(key) > MAX_ENV_KEY_BYTES;
export const envValueTooLong = (value: string): boolean => bytes(value) > MAX_ENV_VALUE_BYTES;

/** Sitios: límites de `nginx/sites.rs`. */
export const MAX_SITE_DOMAINS = 20;
/** El dominio principal da nombre a `blacksmith-certbot@<dominio>.service` (≤ 255 caracteres). */
export const MAX_PRIMARY_DOMAIN_LENGTH = 255 - 'blacksmith-certbot@.service'.length;

/** Nombre de host (`valid_domain`): etiquetas de `a-z0-9-` (1-63, sin guion en los extremos), al
 * menos dos, con un TLD que no sea numérico (una IP no pasa por dominio); o `localhost`. */
export function validDomain(domain: string): boolean {
    if (domain === 'localhost') return true;
    const labels = domain.split('.');
    return (
        domain.length <= 253 &&
        labels.length >= 2 &&
        labels.every((label) => /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(label)) &&
        /[a-z]/.test(labels.at(-1) ?? '')
    );
}

/** Correo para Let's Encrypt (`valid_email`). Sin guion delante: certbot lo leería como opción. */
export function validEmail(email: string): boolean {
    const at = email.lastIndexOf('@');
    if (at < 1) return false;
    const user = email.slice(0, at);
    const host = email.slice(at + 1).toLowerCase();
    return (
        user.length <= 64 &&
        !user.startsWith('-') &&
        /^[A-Za-z0-9._%+-]+$/.test(user) &&
        host !== 'localhost' &&
        validDomain(host)
    );
}
