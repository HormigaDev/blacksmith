import { expect, test } from 'vitest';

import { isLoopbackHost } from './target';

test.each(['localhost', 'LOCALHOST', ' localhost ', 'localhost.', '127.0.0.1', '127.1.2.3', '::1', '[::1]'])(
    '%s es este equipo',
    (host) => {
        expect(isLoopbackHost(host)).toBe(true);
    },
);

test.each([
    'vps.example.com',
    'localhost.evil.com',
    '127.0.0.1.nip.io',
    '10.0.0.1',
    '0.0.0.0',
    '127.0.0.256',
    '::',
    '',
])('%s no es este equipo', (host) => {
    expect(isLoopbackHost(host)).toBe(false);
});
