/**
 * `true` si el host es este mismo equipo: `localhost` o una IP de loopback (127.0.0.0/8, ::1).
 * Misma regla que la guarda del agente (agent/src/http/guard.rs). Para estos hosts no hace falta
 * túnel: el cliente habla directamente con el agente local.
 */
export function isLoopbackHost(raw: string): boolean {
    const host = raw
        .trim()
        .toLowerCase()
        .replace(/^\[(.*)\]$/, '$1')
        .replace(/\.$/, '');
    if (host === 'localhost' || host === '::1' || host === '0:0:0:0:0:0:0:1') return true;
    const octets = host.split('.');
    return (
        octets.length === 4 &&
        octets[0] === '127' &&
        octets.every((octet) => /^\d{1,3}$/.test(octet) && Number(octet) <= 255)
    );
}
