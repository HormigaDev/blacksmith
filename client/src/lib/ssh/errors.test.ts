import { describe, expect, test } from 'vitest';

import { toTunnelError, TunnelError } from './errors';

describe('toTunnelError', () => {
    test('conserva el código y los detalles de la llave del servidor', () => {
        const error = toTunnelError({
            code: 'HOST_KEY_UNKNOWN',
            message: 'la llave del servidor es desconocida',
            details: { algorithm: 'ssh-ed25519', fingerprint: 'SHA256:abc' },
        });
        expect(error).toBeInstanceOf(TunnelError);
        expect(error.code).toBe('HOST_KEY_UNKNOWN');
        expect(error.hostKey).toEqual({ algorithm: 'ssh-ed25519', fingerprint: 'SHA256:abc' });
    });

    test('lee el puerto ocupado y el campo inválido', () => {
        expect(toTunnelError({ code: 'PORT_IN_USE', message: 'x', details: { port: 4040 } }).port).toBe(4040);
        expect(toTunnelError({ code: 'INVALID_TARGET', message: 'x', details: { field: 'host' } }).field).toBe('host');
        expect(
            toTunnelError({ code: 'INVALID_TARGET', message: 'x', details: { field: 'otro' } }).field,
        ).toBeUndefined();
    });

    test('cualquier otra cosa es INTERNAL', () => {
        expect(toTunnelError({ code: 'DESCONOCIDO' }).code).toBe('INTERNAL');
        expect(toTunnelError(new Error('boom')).code).toBe('INTERNAL');
        expect(toTunnelError('texto').code).toBe('INTERNAL');
        expect(toTunnelError(null).code).toBe('INTERNAL');
    });
});
