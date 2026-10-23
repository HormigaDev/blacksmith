<!--
  Grupo (app): pantallas de la aplicación ya configurada (dashboard, servidores…).
  Los flujos a pantalla completa, como la configuración inicial, van en su propio grupo,
  p. ej. src/routes/(setup)/, con su propio +layout.svelte.
-->
<script lang="ts">
    import type { Snippet } from 'svelte';

    import AppShell from '$lib/components/layout/AppShell.svelte';
    import { Button, ConfirmDialog, Dialog, Input } from '$lib/components/ui';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';
    import { github, servers } from '$lib/stores';
    import { toast } from '$lib/toast';

    let { children }: { children: Snippet } = $props();

    // Servidores guardados, y aviso si un túnel se pierde (red caída, servidor reiniciado).
    $effect(() => {
        servers.load().catch((error: unknown) => toast.error(describeError(error)));
    });
    $effect(() => servers.watch());
    // GitHub se carga una vez para toda la aplicación (cuenta y, si la hay, sus repositorios).
    $effect(() => {
        if (!github.loaded) void github.load();
    });

    // Conexión desde cualquier pantalla: confirmar la huella de un servidor nuevo o pedir la
    // passphrase de la llave.
    let passphrase = $state('');

    const trusting = $derived(servers.pendingTrust && servers.find(servers.pendingTrust.id));
    const asking = $derived(servers.pendingPassphrase && servers.find(servers.pendingPassphrase.id));

    // El diálogo refleja el estado pendiente; cerrarlo lo descarta.
    const trustOpen = {
        get value() {
            return servers.pendingTrust !== null;
        },
        set value(open: boolean) {
            if (!open) servers.pendingTrust = null;
        },
    };
    const passphraseOpen = {
        get value() {
            return servers.pendingPassphrase !== null;
        },
        set value(open: boolean) {
            if (!open) {
                servers.pendingPassphrase = null;
                passphrase = '';
            }
        },
    };

    async function submitPassphrase(event: SubmitEvent): Promise<void> {
        event.preventDefault();
        const pending = servers.pendingPassphrase;
        if (!pending) return;
        const value = passphrase;
        servers.pendingPassphrase = null;
        passphrase = '';
        await servers.connect(pending.id, { passphrase: value });
    }
</script>

<AppShell>
    {@render children()}
</AppShell>

<ConfirmDialog
    bind:open={trustOpen.value}
    title={t('servers.connect.trust.title')}
    description={t('servers.connect.trust.description', { host: trusting?.profile.host ?? '' })}
    confirmLabel={t('servers.connect.trust.confirm')}
    onConfirm={() => servers.trust()}
>
    {#if servers.pendingTrust}
        <div class="fingerprint">
            <p class="fingerprint-label">
                {t('servers.connect.trust.fingerprint', { algorithm: servers.pendingTrust.hostKey.algorithm })}
            </p>
            <code class="fingerprint-value">{servers.pendingTrust.hostKey.fingerprint}</code>
            <p class="fingerprint-hint">{t('servers.connect.trust.verifyHint')}</p>
        </div>
    {/if}
</ConfirmDialog>

<Dialog
    bind:open={passphraseOpen.value}
    title={t('servers.passphrase.title', { host: asking?.profile.host ?? '' })}
    description={t('servers.passphrase.description')}
    size="sm"
>
    <form class="passphrase" novalidate onsubmit={submitPassphrase}>
        <Input
            label={t('servers.connect.passphrase')}
            type="password"
            autocomplete="off"
            error={servers.pendingPassphrase?.invalid ? t('servers.passphrase.invalid') : undefined}
            bind:value={passphrase}
        />
        <div class="passphrase-actions">
            <Button type="submit" variant="primary">{t('servers.card.connect')}</Button>
        </div>
    </form>
</Dialog>

<style>
    .fingerprint {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
        padding: var(--space-3);
        background-color: var(--color-bg-subtle);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
    }

    .fingerprint-label,
    .fingerprint-hint {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .fingerprint-value {
        font-size: var(--font-size-sm);
        overflow-wrap: anywhere;
        color: var(--color-text);
    }

    .passphrase {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
    }

    .passphrase-actions {
        display: flex;
        justify-content: flex-end;
    }
</style>
