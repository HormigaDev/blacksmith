<script lang="ts">
    import LayoutGridIcon from '@lucide/svelte/icons/layout-grid';
    import ListIcon from '@lucide/svelte/icons/list';
    import PlusIcon from '@lucide/svelte/icons/plus';

    import { APP_NAME } from '$lib/app';
    import AddServerForm from '$lib/components/servers/AddServerForm.svelte';
    import ServerList from '$lib/components/servers/ServerList.svelte';
    import { Button, Dialog, ToggleGroup } from '$lib/components/ui';
    import { t } from '$lib/i18n';
    import { servers } from '$lib/stores';

    let addOpen = $state(false);

    const views = $derived([
        { value: 'list' as const, label: t('servers.view.list'), icon: ListIcon },
        { value: 'grid' as const, label: t('servers.view.grid'), icon: LayoutGridIcon },
    ]);
</script>

<svelte:head>
    <title>{t('dashboard.title')} · {APP_NAME}</title>
</svelte:head>

{#if servers.entries.length > 0}
    <section class="list">
        <header class="list-header">
            <h1 class="list-title">{t('servers.title')}</h1>
            <div class="list-tools">
                <ToggleGroup
                    label={t('servers.view.label')}
                    options={views}
                    iconOnly
                    size="sm"
                    bind:value={() => servers.view, (view) => (servers.view = view)}
                />
                <Button variant="primary" onclick={() => (addOpen = true)}>
                    <PlusIcon />
                    {t('servers.add.open')}
                </Button>
            </div>
        </header>
        <ServerList
            entries={servers.entries}
            view={servers.view}
            onMove={(from, to) => servers.move(from, to)}
            onCommit={() => servers.saveOrder()}
            onConnect={(id) => servers.connect(id)}
            onDisconnect={(id) => servers.disconnect(id)}
            onRemove={(id) => servers.remove(id)}
            onForgetHostKey={(id) => servers.forgetHostKey(id)}
        />
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

<style>
    .list {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .list-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
        margin-bottom: var(--space-2);
    }

    .list-tools {
        display: flex;
        align-items: center;
        gap: var(--space-3);
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
</style>
