<!--
    Alta de un servidor: destino SSH + llave privada (archivo, pegada o la de ~/.ssh).

    Rust valida la llave, la guarda cifrada y registra el servidor. A partir de ahí el servidor queda
    en la lista aunque la conexión falle (p. ej. el agente aún no está instalado): la conexión la
    gestiona `servers` ($lib/servers/servers.svelte), no este formulario.

    Si el host es este equipo (`localhost`, 127.x.x.x, ::1), no hace falta túnel ni llave: solo se
    pide el puerto del agente y el servidor se registra como local.
-->
<script lang="ts">
    import { untrack } from 'svelte';

    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';
    import {
        addLocalServer,
        AGENT_PORT,
        isLoopbackHost,
        type KeySource,
        saveKey,
        type ServerProfile,
        type SshTarget,
        toTunnelError,
    } from '$lib/ssh';

    import Button from '../ui/Button.svelte';
    import Checkbox from '../ui/Checkbox.svelte';
    import FilePicker from '../ui/FilePicker.svelte';
    import Input from '../ui/Input.svelte';
    import Textarea from '../ui/Textarea.svelte';
    import ToggleGroup from '../ui/ToggleGroup.svelte';

    type Source = 'file' | 'inline' | 'default';
    type Field = 'host' | 'port' | 'username' | 'key' | 'passphrase';

    interface Props {
        initial?: Partial<SshTarget>;
        /** El servidor quedó guardado. `passphrase` sirve para la primera conexión si no se recordó. */
        onSaved?: (server: ServerProfile, passphrase: string | undefined) => void;
    }

    let { initial, onSaved }: Props = $props();

    // Valores iniciales: se leen una vez; después el formulario es dueño de su estado.
    const start = untrack(() => initial);
    let host = $state(start?.host ?? '');
    let port = $state<number | null>(start?.port ?? 22);
    let username = $state(start?.username ?? '');
    let source = $state<Source>('file');
    let keyPaths = $state<string[]>([]);
    let keyContent = $state('');
    let passphrase = $state('');
    let rememberPassphrase = $state(false);
    let askPassphrase = $state(false);
    let agentPort = $state<number | null>(AGENT_PORT);

    let busy = $state(false);
    let fieldErrors = $state<Partial<Record<Field, string>>>({});
    let formError = $state<string>();

    const target = $derived<Required<SshTarget>>({ host: host.trim(), port: port ?? 22, username: username.trim() });
    /** Este mismo equipo: sin túnel ni llave. */
    const local = $derived(isLoopbackHost(host));

    const sources = $derived([
        { value: 'file' as const, label: t('servers.connect.sources.file') },
        { value: 'inline' as const, label: t('servers.connect.sources.inline') },
        { value: 'default' as const, label: t('servers.connect.sources.default') },
    ]);

    function keySource(): KeySource | undefined {
        if (source === 'default') return { kind: 'default' };
        if (source === 'file') return keyPaths[0] ? { kind: 'file', path: keyPaths[0] } : undefined;
        return keyContent.trim() ? { kind: 'inline', content: keyContent } : undefined;
    }

    const validPort = (value: number | null) =>
        value !== null && Number.isInteger(value) && value >= 1 && value <= 65535;

    function validateLocal(): boolean {
        fieldErrors = validPort(agentPort) ? {} : { port: t('errors.tunnel.INVALID_TARGET') };
        return validPort(agentPort);
    }

    function validate(): KeySource | undefined {
        const required = t('servers.connect.required');
        const errors: Partial<Record<Field, string>> = {};
        if (!target.host) errors.host = required;
        if (!target.username) errors.username = required;
        if (!validPort(port)) errors.port = t('errors.tunnel.INVALID_TARGET');
        const keySourceValue = keySource();
        if (!keySourceValue) errors.key = required;
        fieldErrors = errors;
        return Object.keys(errors).length === 0 ? keySourceValue : undefined;
    }

    function handle(error: unknown): void {
        const failure = toTunnelError(error);
        const message = describeError(failure);
        switch (failure.code) {
            case 'PASSPHRASE_REQUIRED':
            case 'PASSPHRASE_INVALID':
                askPassphrase = true;
                fieldErrors = { passphrase: message };
                break;
            case 'KEY_NOT_FOUND':
            case 'KEY_FILE_UNREADABLE':
            case 'KEY_TOO_LARGE':
            case 'KEY_IS_PUBLIC':
            case 'KEY_INVALID':
            case 'KEY_UNSUPPORTED':
                fieldErrors = { key: message };
                break;
            case 'INVALID_TARGET':
                fieldErrors = { [failure.field ?? 'host']: message };
                break;
            default:
                formError = message;
        }
    }

    async function submitLocal(): Promise<void> {
        if (!validateLocal() || agentPort === null) return;
        busy = true;
        try {
            onSaved?.(await addLocalServer(agentPort), undefined);
        } catch (error) {
            handle(error);
        } finally {
            busy = false;
        }
    }

    async function submit(event: SubmitEvent): Promise<void> {
        event.preventDefault();
        formError = undefined;
        if (local) {
            await submitLocal();
            return;
        }
        const keySourceValue = validate();
        if (!keySourceValue) return;
        busy = true;
        try {
            const saved = await saveKey(target, keySourceValue, { passphrase, rememberPassphrase });
            // Rust ya la tiene cifrada: no se conserva en la memoria de la interfaz.
            keyContent = '';
            onSaved?.(saved.server, passphrase || undefined);
            passphrase = '';
        } catch (error) {
            handle(error);
        } finally {
            busy = false;
        }
    }
</script>

<form class="form" novalidate onsubmit={submit}>
    <div class="row">
        <div class="grow">
            <Input
                label={t('servers.connect.host')}
                hint={t('servers.connect.hostHint')}
                placeholder={t('servers.connect.hostPlaceholder')}
                required
                autocomplete="off"
                spellcheck="false"
                error={fieldErrors.host}
                bind:value={host}
            />
        </div>
        <div class="port">
            {#if local}
                <Input
                    label={t('servers.connect.agentPort')}
                    type="number"
                    min={1}
                    max={65535}
                    required
                    error={fieldErrors.port}
                    bind:value={agentPort}
                />
            {:else}
                <Input
                    label={t('servers.connect.port')}
                    type="number"
                    min={1}
                    max={65535}
                    required
                    error={fieldErrors.port}
                    bind:value={port}
                />
            {/if}
        </div>
    </div>

    {#if local}
        <p class="local-note">{t('servers.connect.localNote')}</p>
    {:else}
        <Input
            label={t('servers.connect.username')}
            placeholder={t('servers.connect.usernamePlaceholder')}
            required
            autocomplete="off"
            spellcheck="false"
            error={fieldErrors.username}
            bind:value={username}
        />

        <div class="key">
            <ToggleGroup label={t('servers.connect.keySource')} options={sources} bind:value={source} />

            {#if source === 'file'}
                <FilePicker
                    label={t('servers.connect.keyFile')}
                    hint={t('servers.connect.storageHint')}
                    required
                    error={fieldErrors.key}
                    bind:value={keyPaths}
                />
            {:else if source === 'inline'}
                <Textarea
                    label={t('servers.connect.keyContent')}
                    hint={t('servers.connect.keyContentHint')}
                    placeholder={t('servers.connect.keyContentPlaceholder')}
                    required
                    monospace
                    rows={6}
                    spellcheck="false"
                    autocomplete="off"
                    error={fieldErrors.key}
                    bind:value={keyContent}
                />
            {:else}
                <p class="note" class:error={Boolean(fieldErrors.key)} role={fieldErrors.key ? 'alert' : undefined}>
                    {fieldErrors.key ?? t('servers.connect.defaultKeyHint')}
                </p>
            {/if}
        </div>

        {#if askPassphrase}
            <Input
                label={t('servers.connect.passphrase')}
                type="password"
                autocomplete="off"
                error={fieldErrors.passphrase}
                bind:value={passphrase}
            />
            <Checkbox label={t('servers.connect.rememberPassphrase')} bind:checked={rememberPassphrase} />
        {/if}
    {/if}

    {#if formError}
        <p class="alert" role="alert">{formError}</p>
    {/if}

    <div class="actions">
        <Button type="submit" variant="primary" loading={busy}>{t('servers.add.submit')}</Button>
    </div>
</form>

<style>
    .form {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
    }

    .row {
        display: flex;
        gap: var(--space-3);
    }

    .grow {
        flex: 1;
        min-width: 0;
    }

    .port {
        width: var(--popover-min-width);
    }

    .key {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .note {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .note.error {
        color: var(--color-danger);
    }

    .local-note {
        padding: var(--space-3);
        font-size: var(--font-size-sm);
        color: var(--color-text);
        background-color: var(--color-info-subtle);
        border-radius: var(--radius-md);
    }

    .alert {
        padding: var(--space-3);
        font-size: var(--font-size-sm);
        color: var(--color-danger);
        background-color: var(--color-danger-subtle);
        border-radius: var(--radius-md);
    }

    .actions {
        display: flex;
        justify-content: flex-end;
    }
</style>
