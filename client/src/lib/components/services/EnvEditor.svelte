<!--
    Editor de variables de entorno (clave/valor). Los valores son secretos: se muestran ocultos y
    cada uno se puede revelar. Valida como el agente antes de guardar (clave `[A-Za-z_][A-Za-z0-9_]*`,
    sin repetir; valores sin caracteres de control) y solo permite guardar si hay cambios.

    Con `inherited` (alcance del servicio), lista además las variables globales del servidor y marca
    las que este servicio sobrescribe.
-->
<script lang="ts">
    import PlusIcon from '@lucide/svelte/icons/plus';
    import Trash2Icon from '@lucide/svelte/icons/trash-2';
    import { untrack } from 'svelte';

    import {
        ENV_KEY_PATTERN,
        envKeyTooLong,
        envValueTooLong,
        type EnvVariable,
        hasControlChars,
        MAX_ENV_VARIABLES,
    } from '$lib/agent';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';

    import Badge from '../ui/Badge.svelte';
    import Button from '../ui/Button.svelte';
    import Input from '../ui/Input.svelte';

    interface Props {
        variables: readonly EnvVariable[];
        /** Globales del servidor (solo en el alcance de un servicio). */
        inherited?: readonly EnvVariable[];
        /** Guarda todas las variables. Si lanza, el error se muestra aquí. */
        onSave: (variables: EnvVariable[]) => Promise<void>;
    }

    let { variables, inherited, onSave }: Props = $props();

    interface Row {
        id: number;
        key: string;
        value: string;
    }

    let nextId = 0;
    const toRows = (list: readonly EnvVariable[]): Row[] =>
        list.map(({ key, value }) => ({ id: nextId++, key, value }));
    let rows = $state<Row[]>(untrack(() => toRows(variables)));
    let saved = $state<readonly EnvVariable[]>(untrack(() => variables));
    let busy = $state(false);
    let formError = $state<string>();
    let submitted = $state(false);

    const current = $derived(rows.map(({ key, value }) => ({ key: key.trim(), value })));
    const dirty = $derived(JSON.stringify(current) !== JSON.stringify(saved));
    const errors = $derived.by(() => {
        const seen = new Set<string>();
        return current.map(({ key, value }) => {
            const error: { key?: string; value?: string } = {};
            if (!ENV_KEY_PATTERN.test(key)) error.key = t('services.env.keyInvalid');
            else if (envKeyTooLong(key)) error.key = t('services.env.keyTooLong', { max: 128 });
            else if (seen.has(key)) error.key = t('services.env.keyDuplicate');
            seen.add(key);
            if (hasControlChars(value)) error.value = t('services.env.valueInvalid');
            else if (envValueTooLong(value)) error.value = t('services.env.valueTooLong', { max: 32 });
            return error;
        });
    });
    const valid = $derived(errors.every((error) => !error.key && !error.value));
    const overridden = $derived(new Set(current.map(({ key }) => key)));

    function add(): void {
        rows.push({ id: nextId++, key: '', value: '' });
    }

    function remove(id: number): void {
        rows = rows.filter((row) => row.id !== id);
    }

    function discard(): void {
        rows = toRows(saved);
        submitted = false;
        formError = undefined;
    }

    async function save(event: SubmitEvent): Promise<void> {
        event.preventDefault();
        submitted = true;
        formError = undefined;
        if (!valid) return;
        busy = true;
        try {
            await onSave(current);
            saved = current;
            submitted = false;
        } catch (error) {
            formError = describeError(error);
        } finally {
            busy = false;
        }
    }
</script>

<form class="editor" novalidate onsubmit={save}>
    {#if rows.length === 0}
        <p class="empty">{t('services.env.empty')}</p>
    {:else}
        <ul class="rows">
            {#each rows as row, index (row.id)}
                {@const error = submitted ? errors[index] : undefined}
                <li class="row">
                    <div class="key">
                        <Input
                            label={t('services.env.key')}
                            hideLabel={index > 0}
                            placeholder={t('services.env.keyPlaceholder')}
                            autocomplete="off"
                            spellcheck="false"
                            maxlength={128}
                            error={error?.key}
                            bind:value={row.key}
                        />
                    </div>
                    <div class="value">
                        <Input
                            label={t('services.env.value')}
                            hideLabel={index > 0}
                            type="password"
                            autocomplete="off"
                            error={error?.value}
                            bind:value={row.value}
                        />
                    </div>
                    <div class="remove" class:labelled={index === 0}>
                        <Button
                            variant="ghost"
                            iconOnly
                            aria-label={t('services.env.remove', { key: row.key || '…' })}
                            onclick={() => remove(row.id)}
                        >
                            <Trash2Icon />
                        </Button>
                    </div>
                </li>
            {/each}
        </ul>
    {/if}

    {#if formError}<p class="alert" role="alert">{formError}</p>{/if}
    {#if rows.length >= MAX_ENV_VARIABLES}
        <p class="note">{t('services.env.tooMany', { max: MAX_ENV_VARIABLES })}</p>
    {/if}

    <div class="actions">
        <Button onclick={add} disabled={rows.length >= MAX_ENV_VARIABLES}>
            <PlusIcon />
            {t('services.env.add')}
        </Button>
        <span class="spacer"></span>
        {#if dirty}<Button variant="ghost" onclick={discard}>{t('services.env.discard')}</Button>{/if}
        <Button type="submit" variant="primary" loading={busy} disabled={!dirty}>{t('services.env.save')}</Button>
    </div>
</form>

{#if inherited}
    <section class="inherited" aria-labelledby="inherited-title">
        <h3 id="inherited-title" class="inherited-title">{t('services.env.inheritedTitle')}</h3>
        {#if inherited.length === 0}
            <p class="empty">{t('services.env.inheritedEmpty')}</p>
        {:else}
            <ul class="inherited-list">
                {#each inherited as variable (variable.key)}
                    <li class="inherited-item" class:overridden={overridden.has(variable.key)}>
                        <code class="inherited-key">{variable.key}</code>
                        {#if overridden.has(variable.key)}
                            <Badge variant="warning">{t('services.env.overridden')}</Badge>
                        {/if}
                    </li>
                {/each}
            </ul>
        {/if}
    </section>
{/if}

<style>
    .note {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .editor {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .rows {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        padding: var(--space-0);
        margin: var(--space-0);
        list-style: none;
    }

    .row {
        display: grid;
        grid-template-columns: minmax(0, 2fr) minmax(0, 3fr) auto;
        align-items: start;
        gap: var(--space-2);
    }

    /* Alinea el botón con los campos de la primera fila, que llevan etiqueta visible. */
    .remove.labelled {
        padding-top: calc(var(--font-size-sm) * var(--line-height-normal) + var(--space-1));
    }

    .empty {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }

    .actions {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-2);
    }

    .spacer {
        flex: 1;
    }

    .inherited {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        padding-top: var(--space-4);
        margin-top: var(--space-4);
        border-top: var(--border-width) solid var(--color-border);
    }

    .inherited-title {
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-semibold);
    }

    .inherited-list {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-2);
        padding: var(--space-0);
        margin: var(--space-0);
        list-style: none;
    }

    .inherited-item {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-1) var(--space-2);
        background-color: var(--color-bg-subtle);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
    }

    .inherited-item.overridden .inherited-key {
        text-decoration: line-through;
        color: var(--color-text-muted);
    }

    .inherited-key {
        font-size: var(--font-size-xs);
    }
</style>
