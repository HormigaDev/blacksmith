import { describe, expect, test } from 'vitest';

import { serverAddress, serverHref } from './address';

describe('serverAddress', () => {
    test('SSH: sin el puerto por defecto', () => {
        expect(serverAddress({ id: 'x', kind: 'ssh', host: 'vps.example.com', port: 22, username: 'deploy' })).toBe(
            'deploy@vps.example.com',
        );
        expect(serverAddress({ id: 'x', kind: 'ssh', host: '10.0.0.5', port: 2222, username: 'root' })).toBe(
            'root@10.0.0.5:2222',
        );
    });

    test('local: localhost y el puerto del agente', () => {
        expect(serverAddress({ id: 'local:4040', kind: 'local', host: 'localhost', port: 4040, username: '' })).toBe(
            'localhost:4040',
        );
    });
});

describe('serverHref', () => {
    test('codifica el id y se recupera tal cual', () => {
        for (const id of ['deploy@vps.example.com:22', 'local:4040', 'a/b?c#d %']) {
            const href = serverHref(id);
            expect(href.startsWith('/servers/')).toBe(true);
            // Un único segmento: ni `/`, ni `?`, ni `#` del id escapan de él.
            expect(href.slice('/servers/'.length)).not.toMatch(/[/?#]/);
            expect(decodeURIComponent(href.slice('/servers/'.length))).toBe(id);
        }
    });
});
