<!--
    Conexión con GitHub:
    - Sin conectar: con la App del proyecto (por defecto) o, para quien lo prefiera, una propia.
    - Conectando (Device Flow): el código que hay que confirmar en GitHub.
    - Conectado: la cuenta, dar acceso a más repositorios y desconectar (con confirmación).
    Solo presenta: el estado y las acciones llegan por props (ver `GitHubMenu`).
-->
<script lang="ts">
    import ExternalLinkIcon from '@lucide/svelte/icons/external-link';
    import { untrack } from 'svelte';

    import type { GitHubSettings, GitHubUser, LoginStart } from '$lib/github';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';

    import Button from '../ui/Button.svelte';
    import ConfirmDialog from '../ui/ConfirmDialog.svelte';
    import Input from '../ui/Input.svelte';
    import Spinner from '../ui/Spinner.svelte';

    interface Props {
        settings: GitHubSettings | null;
        /** Se usa una App propia en lugar de la del proyecto. */
        custom?: boolean;
        user: GitHubUser | null;
        pending: LoginStart | null;
        busy?: boolean;
        error?: string | null;
        onConfigure: (settings: GitHubSettings) => Promise<void>;
        onUseDefault: () => Promise<void>;
        onConnect: () => void;
        onCancel: () => void;
        onOpenVerification: () => void;
        onInstall: () => void;
        onDisconnect: () => Promise<void>;
    }

    let {
        settings,
        custom = false,
        user,
        pending,
        busy = false,
        error = null,
        onConfigure,
        onUseDefault,
        onConnect,
        onCancel,
        onOpenVerification,
        onInstall,
        onDisconnect,
    }: Props = $props();

    let editing = $state(false);
    let clientId = $state(untrack(() => (custom ? (settings?.clientId ?? '') : '')));
    let appSlug = $state(untrack(() => (custom ? (settings?.appSlug ?? '') : '')));
    let fieldErrors = $state<{ clientId?: string; appSlug?: string }>({});
    let formError = $state<string>();
    let saving = $state(false);
    let disconnectOpen = $state(false);

    async function save(event: SubmitEvent): Promise<void> {
        event.preventDefault();
        formError = undefined;
        const errors: typeof fieldErrors = {};
        const required = t('github.configure.required');
        const invalid = t('github.configure.invalid');
        if (!clientId.trim()) errors.clientId = required;
        else if (!/^[A-Za-z0-9.]{8,64}$/.test(clientId.trim())) errors.clientId = invalid;
        if (!appSlug.trim()) errors.appSlug = required;
        else if (!/^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/.test(appSlug.trim())) errors.appSlug = invalid;
        fieldErrors = errors;
        if (Object.keys(errors).length > 0) return;
        saving = true;
        try {
            await onConfigure({ clientId: clientId.trim(), appSlug: appSlug.trim() });
            editing = false;
        } catch (caught) {
            formError = describeError(caught);
        } finally {
            saving = false;
        }
    }
</script>

<div class="panel">
    {#if editing}
        <form class="form" novalidate onsubmit={save}>
            <p class="note">{t('github.configure.description')}</p>
            <Input
                label={t('github.configure.clientId')}
                hint={t('github.configure.clientIdHint')}
                placeholder={t('github.configure.clientIdPlaceholder')}
                required
                autocomplete="off"
                spellcheck="false"
                error={fieldErrors.clientId}
                bind:value={clientId}
            />
            <Input
                label={t('github.configure.appSlug')}
                hint={t('github.configure.appSlugHint')}
                placeholder={t('github.configure.appSlugPlaceholder')}
                required
                autocomplete="off"
                spellcheck="false"
                error={fieldErrors.appSlug}
                bind:value={appSlug}
            />
            {#if formError}<p class="alert" role="alert">{formError}</p>{/if}
            <div class="actions">
                <Button variant="ghost" onclick={() => (editing = false)}>{t('common.cancel')}</Button>
                <Button type="submit" variant="primary" loading={saving}>{t('github.configure.save')}</Button>
            </div>
        </form>
    {:else if pending}
        <div class="waiting">
            <p class="note">{t('github.waiting.description')}</p>
            <p class="code" aria-label={pending.userCode}>{pending.userCode}</p>
            <Spinner label={t('github.waiting.title')} />
            <div class="actions">
                <Button variant="ghost" onclick={onCancel}>{t('github.waiting.cancel')}</Button>
                <Button onclick={onOpenVerification}>
                    <ExternalLinkIcon />
                    {t('github.waiting.open')}
                </Button>
            </div>
        </div>
    {:else if user}
        <div class="connected">
            <div class="account">
                {#if user.avatarUrl}<img class="avatar" src={user.avatarUrl} alt="" />{/if}
                <div>
                    <p class="login">{user.login}</p>
                    {#if user.name}<p class="note">{user.name}</p>{/if}
                </div>
            </div>
            <div class="actions">
                <Button onclick={onInstall}>
                    <ExternalLinkIcon />
                    {t('github.connected.install')}
                </Button>
                <Button variant="danger" onclick={() => (disconnectOpen = true)}>
                    {t('github.connected.disconnect')}
                </Button>
            </div>
        </div>
    {:else}
        <div class="connect">
            {#if error}<p class="alert" role="alert">{error}</p>{/if}
            <Button variant="primary" fullWidth loading={busy} onclick={onConnect}>{t('github.connect')}</Button>
            {#if custom}
                <p class="note">{t('github.customApp', { slug: settings?.appSlug ?? '' })}</p>
                <div class="links">
                    <Button size="sm" variant="ghost" onclick={() => (editing = true)}>
                        {t('github.configure.change')}
                    </Button>
                    <Button size="sm" variant="ghost" onclick={onUseDefault}>{t('github.useDefault')}</Button>
                </div>
            {:else}
                <div class="links">
                    <Button size="sm" variant="ghost" onclick={() => (editing = true)}>{t('github.useOwn')}</Button>
                </div>
            {/if}
        </div>
    {/if}
</div>

<ConfirmDialog
    bind:open={disconnectOpen}
    variant="danger"
    title={t('github.connected.disconnectTitle')}
    description={t('github.connected.disconnectDescription')}
    confirmLabel={t('github.connected.disconnect')}
    onConfirm={onDisconnect}
/>

<style>
    .panel,
    .form,
    .waiting,
    .connected,
    .connect {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
    }

    .waiting {
        align-items: center;
        text-align: center;
    }

    .login {
        font-weight: var(--font-weight-semibold);
    }

    .links {
        display: flex;
        justify-content: center;
        gap: var(--space-2);
    }

    .note {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .code {
        padding: var(--space-3) var(--space-5);
        font-family: var(--font-mono);
        font-size: var(--font-size-xl);
        font-weight: var(--font-weight-semibold);
        letter-spacing: var(--space-1);
        background-color: var(--color-bg-subtle);
        border: var(--border-width) dashed var(--color-border-strong);
        border-radius: var(--radius-md);
        user-select: all;
    }

    .account {
        display: flex;
        align-items: center;
        gap: var(--space-3);
    }

    .avatar {
        width: var(--control-height-md);
        height: var(--control-height-md);
        border-radius: var(--radius-full);
    }

    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }

    .actions {
        display: flex;
        flex-wrap: wrap;
        justify-content: flex-end;
        gap: var(--space-2);
    }
</style>
