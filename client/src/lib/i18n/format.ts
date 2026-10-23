// Formato de números, fechas y tiempos relativos en el idioma activo. Nunca se formatea a
// mano (`toFixed`, `toLocaleString()` sin idioma, concatenar "hace X min"): se usa esto.
// Son reactivas: leen `i18n.current`, así que la plantilla se actualiza al cambiar de idioma.

import { i18n } from './i18n.svelte';

const cache = new Map<string, Intl.NumberFormat | Intl.DateTimeFormat | Intl.RelativeTimeFormat>();

function cached<T extends Intl.NumberFormat | Intl.DateTimeFormat | Intl.RelativeTimeFormat>(
    kind: string,
    options: object | undefined,
    create: (locale: string) => T,
): T {
    const locale = i18n.current.intl;
    const key = `${kind}|${locale}|${JSON.stringify(options ?? {})}`;
    let formatter = cache.get(key) as T | undefined;
    if (formatter === undefined) {
        formatter = create(locale);
        cache.set(key, formatter);
    }
    return formatter;
}

export function formatNumber(value: number, options?: Intl.NumberFormatOptions): string {
    return cached('number', options, (locale) => new Intl.NumberFormat(locale, options)).format(value);
}

/** Bytes en la unidad más legible: `formatBytes(1536)` → "1,5 kB". */
export function formatBytes(bytes: number): string {
    const units = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte'] as const;
    let value = bytes;
    let unit = 0;
    while (Math.abs(value) >= 1024 && unit < units.length - 1) {
        value /= 1024;
        unit += 1;
    }
    return formatNumber(value, {
        style: 'unit',
        unit: units[unit],
        unitDisplay: 'short',
        maximumFractionDigits: 1,
    });
}

export function formatPercent(ratio: number, maximumFractionDigits = 1): string {
    return formatNumber(ratio, { style: 'percent', maximumFractionDigits });
}

export function formatDateTime(
    date: Date | number,
    options: Intl.DateTimeFormatOptions = { dateStyle: 'medium', timeStyle: 'short' },
): string {
    return cached('date', options, (locale) => new Intl.DateTimeFormat(locale, options)).format(date);
}

const DURATION_UNITS: readonly [Intl.NumberFormatOptions['unit'] & string, number][] = [
    ['day', 24 * 3600],
    ['hour', 3600],
    ['minute', 60],
    ['second', 1],
];

/**
 * Duración con las dos unidades más grandes: "3 d 4 h", "2 h 5 min", "42 s". Para tiempos en
 * marcha y similares (no relativos a ahora).
 */
export function formatDuration(totalSeconds: number): string {
    const seconds = Math.max(0, Math.floor(totalSeconds));
    const parts: string[] = [];
    let rest = seconds;
    for (const [unit, size] of DURATION_UNITS) {
        const amount = Math.floor(rest / size);
        if (amount > 0 || (unit === 'second' && parts.length === 0)) {
            parts.push(formatNumber(amount, { style: 'unit', unit, unitDisplay: 'short' }));
            rest -= amount * size;
        }
        if (parts.length === 2) break;
    }
    return parts.join(' ');
}

const RELATIVE_UNITS: readonly [Intl.RelativeTimeFormatUnit, number][] = [
    ['year', 365 * 24 * 3600],
    ['month', 30 * 24 * 3600],
    ['day', 24 * 3600],
    ['hour', 3600],
    ['minute', 60],
    ['second', 1],
];

/** Tiempo relativo a `now`: "hace 5 minutos", "in 2 hours". */
export function formatRelativeTime(date: Date | number, now: Date | number = Date.now()): string {
    const seconds = (Number(date) - Number(now)) / 1000;
    const formatter = cached(
        'relative',
        undefined,
        (locale) => new Intl.RelativeTimeFormat(locale, { numeric: 'auto' }),
    );
    for (const [unit, size] of RELATIVE_UNITS) {
        if (Math.abs(seconds) >= size || unit === 'second') {
            return formatter.format(Math.round(seconds / size), unit);
        }
    }
    return formatter.format(0, 'second');
}

const lists = new Map<string, Intl.ListFormat>();

/** Lista natural según el idioma: `formatList(['.pem', '.key', '.pub'])` → ".pem, .key y .pub". */
export function formatList(items: readonly string[], type: Intl.ListFormatType = 'conjunction'): string {
    const locale = i18n.current.intl;
    const key = `${locale}|${type}`;
    let formatter = lists.get(key);
    if (formatter === undefined) {
        formatter = new Intl.ListFormat(locale, { type, style: 'long' });
        lists.set(key, formatter);
    }
    return formatter.format(items);
}
