#!/usr/bin/env node
// Hace cumplir la organización de archivos de client/docs/testing.md §5. Uso: pnpm check:structure
//
// 1. Las historias (`*.stories.svelte`) siempre van en una carpeta `stories/`, junto a la carpeta
//    de sus componentes.
// 2. Una carpeta con más de 5 archivos de test los agrupa en una carpeta `tests/`. Con 5 o menos
//    pueden ir junto al código que prueban.

import { readdirSync } from 'node:fs';
import { basename, dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const CLIENT = fileURLToPath(new URL('..', import.meta.url));
const SRC = join(CLIENT, 'src');
const MAX_LOOSE_TESTS = 5;

const isTest = (name) => /\.test\.ts$/.test(name);
const isStory = (name) => name.endsWith('.stories.svelte');

const files = readdirSync(SRC, { recursive: true, encoding: 'utf8' }).map((file) => join(SRC, file));
const errors = [];

for (const file of files.filter((path) => isStory(basename(path)))) {
    if (basename(dirname(file)) !== 'stories') {
        errors.push(`${relative(CLIENT, file)}: las historias van en una carpeta stories/`);
    }
}

const testsByDir = new Map();
for (const file of files.filter((path) => isTest(basename(path)))) {
    const dir = dirname(file);
    testsByDir.set(dir, [...(testsByDir.get(dir) ?? []), file]);
}
for (const [dir, tests] of testsByDir) {
    if (basename(dir) !== 'tests' && tests.length > MAX_LOOSE_TESTS) {
        errors.push(
            `${relative(CLIENT, dir)}/: ${tests.length} archivos de test (máximo ${MAX_LOOSE_TESTS} sueltos); muévelos a ${relative(CLIENT, dir)}/tests/`,
        );
    }
}

if (errors.length > 0) {
    console.error(`check:structure: ${errors.length} problema(s). Reglas: client/docs/testing.md\n`);
    for (const error of errors) console.error(`  ${error}`);
    process.exit(1);
}
console.log('check:structure: OK');
