<script lang="ts">
    import PlusIcon from '@lucide/svelte/icons/plus';

    import { APP_NAME } from '$lib/app';
    import AddServerForm from '$lib/components/servers/AddServerForm.svelte';
    import ServerCard from '$lib/components/servers/ServerCard.svelte';
    import { Button, ConfirmDialog, Dialog, Input } from '$lib/components/ui';
    import { t } from '$lib/i18n';
    import { servers } from '$lib/servers/servers.svelte';

    let addOpen = $state(false);
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

<svelte:head>
    <title>{t('dashboard.title')} · {APP_NAME}</title>
</svelte:head>

{#if servers.entries.length > 0}
    <section class="list">
        <header class="list-header">
            <h1 class="list-title">{t('servers.title')}</h1>
            <Button variant="primary" onclick={() => (addOpen = true)}>
                <PlusIcon />
                {t('servers.add.open')}
            </Button>
        </header>
        {#each servers.entries as entry (entry.profile.id)}
            <ServerCard
                {entry}
                onConnect={() => servers.connect(entry.profile.id)}
                onDisconnect={() => servers.disconnect(entry.profile.id)}
                onRemove={() => servers.remove(entry.profile.id)}
                onForgetHostKey={() => servers.forgetHostKey(entry.profile.id)}
            />
        {/each}
    </section>
{:else if servers.loaded}
    <section class="empty">
        <h1 class="title">{t('servers.empty.title')}</h1>
        <p class="description">{t('servers.empty.description')}</p>
        <div class="cta">
            <Button variant="primary" onclick={() => (addOpen = true)}>
                <PlusIcon />
                {t('servers.add.open')}
            </Button>
        </div>
    </section>
{/if}

<Dialog bind:open={addOpen} title={t('servers.add.title')} description={t('servers.add.description')} size="lg">
    <AddServerForm
        onSaved={(server, firstPassphrase) => {
            servers.added(server);
            addOpen = false;
            void servers.connect(server.id, { passphrase: firstPassphrase });
        }}
    />
</Dialog>

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
    .list {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
        max-width: var(--layout-content-max-width);
    }

    .list-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
        margin-bottom: var(--space-2);
    }

    .list-title {
        font-size: var(--font-size-xl);
    }

    .empty {
        display: grid;
        place-content: center;
        gap: var(--space-2);
        min-height: 100%;
        text-align: center;
    }

    .title {
        font-size: var(--font-size-xl);
    }

    .description {
        color: var(--color-text-muted);
        font-size: var(--font-size-sm);
    }

    .cta {
        margin-top: var(--space-4);
    }

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
