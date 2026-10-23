<!--
    Llave de despliegue del servicio (acceso «Llave de despliegue»): se genera en el servidor (la
    privada nunca sale de allí) y se añade al repositorio como *deploy key* de solo lectura.

    Registrarla con un clic exige el permiso *Administration* de GitHub, que también permite
    borrar repositorios: la App de BBEL no lo pide. Solo se ofrece con una App propia
    (`canRegister`); si aun así GitHub lo rechaza, se pasa al registro manual.
-->
<script lang="ts">
    import CheckIcon from '@lucide/svelte/icons/check';
    import CopyIcon from '@lucide/svelte/icons/copy';
    import ExternalLinkIcon from '@lucide/svelte/icons/external-link';
    import KeyRoundIcon from '@lucide/svelte/icons/key-round';

    import type { DeployKey, RepositoryRef } from '$lib/agent';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';
    import { TunnelError } from '$lib/ssh';

    import Button from '../ui/Button.svelte';
    import Spinner from '../ui/Spinner.svelte';

    interface Props {
        /** `undefined` mientras carga; `null` si aún no se generó. */
        key: DeployKey | null | undefined;
        repository: RepositoryRef;
        /** Hay una App propia conectada (con la que se puede intentar el registro en un clic). */
        canRegister: boolean;
        onCreate: () => Promise<unknown>;
        /** Registra la llave en el repositorio con la API de GitHub. */
        onRegister: () => Promise<void>;
        /** Abre en el navegador la página del repositorio para añadirla a mano. */
        onOpenGitHub: () => void;
    }

    let { key, repository, canRegister, onCreate, onRegister, onOpenGitHub }: Props = $props();

    let busy = $state<'create' | 'register' | null>(null);
    let registered = $state(false);
    let manual = $state(false);
    let copied = $state(false);
    let error = $state<string>();

    const fullName = $derived(`${repository.owner}/${repository.name}`);

    async function create(): Promise<void> {
        busy = 'create';
        error = undefined;
        try {
            await onCreate();
        } catch (caught) {
            error = describeError(caught);
        } finally {
            busy = null;
        }
    }

    async function register(): Promise<void> {
        busy = 'register';
        error = undefined;
        try {
            await onRegister();
            registered = true;
        } catch (caught) {
            if (caught instanceof TunnelError && caught.code === 'GITHUB_PERMISSION_MISSING') manual = true;
            else error = describeError(caught);
        } finally {
            busy = null;
        }
    }

    async function copy(): Promise<void> {
        if (!key) return;
        try {
            await navigator.clipboard.writeText(key.publicKey);
            copied = true;
            setTimeout(() => (copied = false), 2000);
        } catch (caught) {
            error = describeError(caught);
        }
    }
</script>

<div class="panel">
    {#if key === undefined}
        <Spinner label={t('common.loading')} />
    {:else if key === null}
        <p class="note">{t('services.deployKey.missing')}</p>
        <div>
            <Button loading={busy === 'create'} onclick={create}>
                <KeyRoundIcon />
                {t('services.deployKey.create')}
            </Button>
        </div>
    {:else}
        <ol class="steps">
            <li class="step">
                <span class="number" aria-hidden="true">1</span>
                <div class="body">
                    <p>{t('services.deployKey.stepCopy')}</p>
                    <code class="public-key">{key.publicKey}</code>
                    <div class="row">
                        <Button size="sm" onclick={copy}>
                            {#if copied}<CheckIcon />{:else}<CopyIcon />{/if}
                            {copied ? t('services.deployKey.copied') : t('services.deployKey.copy')}
                        </Button>
                        <span class="note">{t('services.deployKey.fingerprint', { fingerprint: key.fingerprint })}</span
                        >
                    </div>
                </div>
            </li>
            <li class="step">
                <span class="number" aria-hidden="true">2</span>
                <div class="body">
                    {#if registered}
                        <p class="success" role="status">
                            <CheckIcon />
                            {t('services.deployKey.registered', { repository: fullName })}
                        </p>
                    {:else if canRegister && !manual}
                        <p>{t('services.deployKey.stepRegister', { repository: fullName })}</p>
                        <div>
                            <Button size="sm" loading={busy === 'register'} onclick={register}>
                                {t('services.deployKey.register')}
                            </Button>
                        </div>
                    {:else}
                        <p>{t('services.deployKey.stepAdd', { repository: fullName })}</p>
                        <div>
                            <Button size="sm" onclick={onOpenGitHub}>
                                <ExternalLinkIcon />
                                {t('services.deployKey.openGitHub')}
                            </Button>
                        </div>
                        <p class="note">
                            {manual ? t('services.deployKey.permissionMissing') : t('services.deployKey.ownAppHint')}
                        </p>
                    {/if}
                </div>
            </li>
            <li class="step">
                <span class="number" aria-hidden="true">3</span>
                <div class="body">
                    <p>{t('services.deployKey.stepDeploy')}</p>
                </div>
            </li>
        </ol>
    {/if}
    {#if error}<p class="alert" role="alert">{error}</p>{/if}
</div>

<style>
    .panel {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .steps {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
        padding: var(--space-0);
        margin: var(--space-0);
        list-style: none;
    }

    .step {
        display: flex;
        gap: var(--space-3);
        font-size: var(--font-size-sm);
    }

    .number {
        display: inline-flex;
        flex-shrink: 0;
        align-items: center;
        justify-content: center;
        width: var(--icon-size-lg);
        height: var(--icon-size-lg);
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-semibold);
        color: var(--color-text-muted);
        background-color: var(--color-bg-subtle);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-full);
    }

    .body {
        display: flex;
        flex: 1;
        flex-direction: column;
        gap: var(--space-2);
        min-width: 0;
    }

    .row {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-3);
    }

    .public-key {
        display: block;
        padding: var(--space-2) var(--space-3);
        font-size: var(--font-size-xs);
        overflow-wrap: anywhere;
        user-select: all;
        background-color: var(--color-bg-subtle);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
    }

    .note {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
        overflow-wrap: anywhere;
    }

    .success {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
        color: var(--color-success);
    }

    .success :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
    }

    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }
</style>
