<!-- Pestaña Variables: las del servicio, con las globales del servidor que hereda. -->
<script lang="ts">
    import { untrack } from 'svelte';

    import { page } from '$app/state';
    import EnvEditor from '$lib/components/services/EnvEditor.svelte';
    import { Button, Panel, Spinner } from '$lib/components/ui';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';
    import { serverHref } from '$lib/servers/address';
    import { servers, serviceStore } from '$lib/stores';
    import { toast } from '$lib/toast';

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const name = $derived(page.params.name ?? '');
    const server = $derived(serverId ? serviceStore.server(serverId) : null);
    const service = $derived(serverId ? serviceStore.service(serverId, name) : null);

    let globalError = $state<string | null>(null);
    function loadGlobal(): void {
        globalError = null;
        server?.loadGlobal().catch((error: unknown) => (globalError = describeError(error)));
    }
    $effect(() => {
        if (server && server.global === null) untrack(loadGlobal);
    });
</script>

{#if service && server && serverId}
    {#if service.error && !service.env}
        <p class="alert" role="alert">{service.error}</p>
    {:else if globalError && !server.global}
        <p class="alert" role="alert">{globalError}</p>
        <div><Button size="sm" onclick={loadGlobal}>{t('common.retry')}</Button></div>
    {:else if !service.env || !server.global}
        <Spinner label={t('common.loading')} />
    {:else}
        <Panel>
            {#snippet actions()}
                <a class="link" href="{serverHref(serverId)}/env">{t('services.env.globalLink')}</a>
            {/snippet}
            {#if !service.env.loadedByUnit || !service.env.globalLoadedByUnit}
                <p class="warning" role="status">{t('services.env.notLoaded')}</p>
            {/if}
            {#key service.env.name}
                <EnvEditor
                    variables={service.env.variables}
                    inherited={server.global.variables}
                    onSave={async (variables) => {
                        await service.saveEnv(variables);
                        toast.success(t('services.env.saved'));
                    }}
                />
            {/key}
        </Panel>
    {/if}
{/if}

<style>
    .link {
        font-size: var(--font-size-sm);
        color: var(--color-accent);
    }

    .warning {
        padding: var(--space-3);
        font-size: var(--font-size-sm);
        background-color: var(--color-warning-subtle);
        border-radius: var(--radius-md);
    }

    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }
</style>
