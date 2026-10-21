import type es from './messages/es';

/** Categorías de `Intl.PluralRules`. `other` es obligatoria. */
export type PluralCategory = Intl.LDMLPluralRule;
export type PluralMessage = { readonly other: string } & {
    readonly [C in Exclude<PluralCategory, 'other'>]?: string;
};

/** Árbol de mensajes del idioma base, con los textos como tipos literales. */
export type Messages = typeof es;

/** Forma de un archivo de traducción: mismo árbol que el base, todo opcional y con textos libres. */
export type Translation = Widen<Messages>;
type Widen<T> = T extends string
    ? string
    : T extends PluralMessage
      ? { readonly [C in keyof T]?: string } & { readonly other: string }
      : { readonly [K in keyof T]?: Widen<T[K]> };

/** Todas las claves válidas: `"dashboard.empty.title"`. */
export type MessageKey = Paths<Messages>;
type Paths<T> = {
    [K in keyof T & string]: T[K] extends string | PluralMessage ? K : `${K}.${Paths<T[K]>}`;
}[keyof T & string];

type ValueAt<T, K extends string> = K extends `${infer Head}.${infer Rest}`
    ? Head extends keyof T
        ? ValueAt<T[Head], Rest>
        : never
    : K extends keyof T
      ? T[K]
      : never;

type Placeholders<S> = S extends `${string}{${infer Name}}${infer Rest}` ? Name | Placeholders<Rest> : never;

export type ParamValue = string | number;

/** Parámetros que exige una clave, deducidos de su texto en español. */
export type MessageParams<K extends MessageKey> =
    ValueAt<Messages, K> extends infer V
        ? V extends PluralMessage
            ? { count: number } & Record<Exclude<Placeholders<V[keyof V]>, 'count'>, ParamValue>
            : V extends string
              ? Record<Placeholders<V>, ParamValue>
              : never
        : never;

/** Sin parámetros, `t(key)`; con parámetros (o plural), `t(key, { ... })` es obligatorio. */
export type TranslateArgs<K extends MessageKey> =
    ValueAt<Messages, K> extends PluralMessage
        ? [params: MessageParams<K>]
        : [Placeholders<ValueAt<Messages, K>>] extends [never]
          ? []
          : [params: MessageParams<K>];
