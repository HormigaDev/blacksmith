import { describe, expect, test } from 'vitest';

import { detectSystemLocale, isLocalePreference, LOCALES } from './locales';

describe('detectSystemLocale', () => {
    test('usa el idioma base si ninguno del sistema está disponible', () => {
        expect(detectSystemLocale(['fr-FR', 'de'])).toBe('es');
        expect(detectSystemLocale([])).toBe('es');
    });

    test('compara por idioma, no por región', () => {
        expect(detectSystemLocale(['es-MX'])).toBe('es');
    });

    test('ignora idiomas registrados pero no disponibles', () => {
        const unavailable = LOCALES.filter((locale) => !locale.available).map((locale) => locale.intl);
        expect(detectSystemLocale(unavailable)).toBe('es');
    });
});

test('solo acepta como preferencia "system" o un idioma disponible', () => {
    expect(isLocalePreference('system')).toBe(true);
    expect(isLocalePreference('es')).toBe(true);
    for (const locale of LOCALES.filter((candidate) => !candidate.available)) {
        expect(isLocalePreference(locale.code)).toBe(false);
    }
    expect(isLocalePreference('xx')).toBe(false);
    expect(isLocalePreference(null)).toBe(false);
});
