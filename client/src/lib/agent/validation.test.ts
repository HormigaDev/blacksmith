import { describe, expect, test } from 'vitest';

import {
    envKeyTooLong,
    envValueTooLong,
    hasControlChars,
    MAX_PRIMARY_DOMAIN_LENGTH,
    validBranch,
    validDomain,
    validEmail,
} from './validation';

// Mismos casos que los tests del agente: si uno cambia, cambia el otro.
describe('validación compartida con el agente', () => {
    test('ramas', () => {
        for (const branch of ['main', 'feature/login', 'release-1.4', 'hotfix_2']) {
            expect(validBranch(branch), branch).toBe(true);
        }
        for (const branch of [
            '',
            'con espacio',
            '-opcion',
            '/raiz',
            'fin/',
            'fin.',
            'a..b',
            'a//b',
            'a@{1}',
            '@',
            '.oculta',
            'x/.oculta',
            'rama.lock',
            'a~1',
            'a^',
            'a:b',
            'a?',
            'a*',
            'a[',
            'a\\b',
            'tab\t',
            'x'.repeat(256),
        ]) {
            expect(validBranch(branch), branch).toBe(false);
        }
    });

    test('dominios', () => {
        for (const domain of ['example.com', 'app.example.com', 'a-b.example.co', 'localhost', 'x1.io']) {
            expect(validDomain(domain), domain).toBe(true);
        }
        for (const domain of ['example', '-a.com', 'a-.com', 'ejemplo.-com', '192.168.1.1', 'a..com', 'A.com']) {
            expect(validDomain(domain), domain).toBe(false);
        }
        expect(validDomain(`${'a'.repeat(64)}.com`)).toBe(false);
        expect(MAX_PRIMARY_DOMAIN_LENGTH).toBe(228);
    });

    test('correos', () => {
        expect(validEmail('ops@example.com')).toBe(true);
        expect(validEmail('a.b+c@sub.example.io')).toBe(true);
        for (const email of ['-d@example.com', 'ops@localhost', 'ops@-x.com', '@example.com', 'ops', 'o ps@x.com']) {
            expect(validEmail(email), email).toBe(false);
        }
    });

    test('variables: límites en bytes, no en caracteres', () => {
        expect(envKeyTooLong('K'.repeat(128))).toBe(false);
        expect(envKeyTooLong('K'.repeat(129))).toBe(true);
        expect(envValueTooLong('ñ'.repeat(16 * 1024))).toBe(false);
        expect(envValueTooLong('ñ'.repeat(16 * 1024 + 1))).toBe(true);
        expect(hasControlChars('a\nb')).toBe(true);
        expect(hasControlChars('ñandú')).toBe(false);
    });
});
