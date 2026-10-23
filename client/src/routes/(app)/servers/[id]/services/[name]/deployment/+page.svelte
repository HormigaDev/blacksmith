<!--
    Pestaña Despliegue: origen y arranque, llave de despliegue (solo si el servicio la usa) y
    despliegues (con la salida del despliegue en vivo mientras dura y tras el último).

    Con acceso «Cuenta de GitHub», al guardar y al desplegar el servidor recibe la cuenta conectada:
    clona y prepara solo, sin pasos en GitHub.
-->
<script lang="ts">
    import { openUrl } from '@tauri-apps/plugin-opener';

    import { untrack } from 'svelte';

    import { page } from '$app/state';
    import { followLogs, isRunning } from '$lib/agent';
    import DeployKeyPanel from '$lib/components/services/DeployKeyPanel.svelte';
    import DeploymentForm from '$lib/components/services/DeploymentForm.svelte';
    import DeployRuns from '$lib/components/services/DeployRuns.svelte';
    import LogViewer from '$lib/components/services/LogViewer.svelte';
    import { Button, Panel, Spinner } from '$lib/components/ui';
    import { addDeployKey, gitHubBranches } from '$lib/github';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';
    import { LogSession } from '$lib/services/log-session.svelte';
    import { github, servers, serviceStore } from '$lib/stores';
    import { toast } from '$lib/toast';

    const entry = $derived(servers.find(page.params.id ?? ''));
    const serverId = $derived(entry?.profile.id);
    const name = $derived(page.params.name ?? '');
    const server = $derived(serverId ? serviceStore.server(serverId) : null);
    const service = $derived(serverId ? serviceStore.service(serverId, name) : null);
    const githubLogin = $derived(github.user?.login ?? null);
    const deployment = $derived(service?.deployment ?? null);
    const repository = $derived(deployment?.repository ?? null);
    const usesKey = $derived(deployment?.access === 'deployKey');
    // Con llave de despliegue el servidor no necesita la cuenta de GitHub.
    const shareLogin = $derived(usesKey ? null : githubLogin);

    const blocked = $derived.by(() => {
        if (!repository) return t('services.deploys.needsRepository');
        if (!deployment?.startCommand && !deployment?.publishDir) return t('services.deploys.needsStart');
        if (usesKey && !service?.key) return t('services.deploys.needsKey');
        return null;
    });

    // Salida del último despliegue (solo la suya, no la de los anteriores): en vivo mientras dura.
    let logs = $state<LogSession>();
    const lastRunId = $derived(service?.runs?.[0]?.id);
    const lastRunStart = $derived(service?.runs?.[0]?.startedAt);
    $effect(() => {
        const id = serverId;
        const current = name;
        if (!id || lastRunId === undefined || lastRunStart === undefined) {
            // Sin despliegues (p. ej. otro servicio): no se queda la salida del anterior.
            logs = undefined;
            return;
        }
        // Un segundo de margen: el registro y el arranque de la unidad no son el mismo instante.
        const since = Math.max(0, lastRunStart - 1);
        const session = new LogSession((onEvent) => followLogs(id, current, onEvent, { deploy: true, since }));
        logs = session;
        void session.start();
        return () => void session.stop();
    });

    $effect(() => {
        // Solo informativo (aviso de repositorios privados): sin él, el formulario funciona igual.
        if (server && server.github === undefined) void server.loadGitHub().catch(() => undefined);
    });
    let keyError = $state<string | null>(null);
    function loadKey(): void {
        keyError = null;
        service?.loadKey().catch((error: unknown) => (keyError = describeError(error)));
    }
    $effect(() => {
        if (service && usesKey && service.key === undefined) untrack(loadKey);
    });

    async function registerKey(): Promise<void> {
        if (!repository || !service?.key) return;
        await addDeployKey(repository.owner, repository.name, `blacksmith-${name}`, service.key.publicKey);
    }

    function openDeployKeys(): void {
        if (!repository) return;
        const path = [repository.owner, repository.name].map(encodeURIComponent).join('/');
        void openUrl(`https://github.com/${path}/settings/keys/new`);
    }

    async function rollback(): Promise<void> {
        try {
            await service?.rollback();
            toast.success(t('services.deploys.rollbackStarted'));
        } catch (error) {
            toast.error(describeError(error));
        }
    }

    async function deploy(): Promise<void> {
        try {
            await service?.deploy(shareLogin);
            toast.success(t('services.deploys.started'));
        } catch (error) {
            toast.error(describeError(error));
        }
    }
</script>

{#if service}
    {#if service.error && !deployment}
        <p class="alert" role="alert">{service.error}</p>
    {:else if !deployment}
        <Spinner label={t('common.loading')} />
    {:else}
        <div class="sections">
            <Panel title={t('services.deployment.title')}>
                {#key deployment.name}
                    <DeploymentForm
                        {deployment}
                        githubConnected={github.user !== null}
                        repositories={github.repositories}
                        repositoriesError={github.repositoriesError}
                        onRetryRepositories={() => void github.loadRepositories(true)}
                        loadBranches={gitHubBranches}
                        onConnectGitHub={() => (github.panelOpen = true)}
                        onSave={async (update) => {
                            await service.saveDeployment(update, update.access === 'account' ? githubLogin : null);
                            toast.success(t('services.deployment.saved'));
                        }}
                    />
                {/key}
            </Panel>

            {#if repository && usesKey}
                <Panel title={t('services.deployKey.title')}>
                    {#if keyError && service.key === undefined}
                        <p class="alert" role="alert">{keyError}</p>
                        <div><Button size="sm" onclick={loadKey}>{t('common.retry')}</Button></div>
                    {:else}
                        <DeployKeyPanel
                            key={service.key}
                            {repository}
                            canRegister={github.custom && github.user !== null}
                            onCreate={() => service.createKey()}
                            onRegister={registerKey}
                            onOpenGitHub={openDeployKeys}
                        />
                    {/if}
                </Panel>
            {/if}

            <Panel title={t('services.deploys.title')}>
                <DeployRuns
                    runs={service.runs}
                    {blocked}
                    activeRelease={service.status?.release}
                    previousRelease={service.status?.previousRelease}
                    onDeploy={deploy}
                    onRollback={rollback}
                    serviceRunning={service.status ? isRunning(service.status.activeState) : null}
                >
                    {#snippet output()}
                        {#if logs}
                            <LogViewer
                                name={`${name} (${t('services.deploys.logs')})`}
                                lines={logs.lines}
                                status={logs.status}
                                endReason={logs.endReason}
                                error={logs.error}
                                onReconnect={() => logs?.start()}
                            />
                        {/if}
                    {/snippet}
                </DeployRuns>
                {#if server && server.github !== undefined && !usesKey}
                    <p class="note">
                        {server.github
                            ? t('services.deploys.githubAccess', { login: server.github.login })
                            : t('services.deploys.publicOnly')}
                    </p>
                {/if}
            </Panel>
        </div>
    {/if}
{/if}

<style>
    .sections {
        display: flex;
        flex-direction: column;
        gap: var(--space-5);
    }

    .note {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }
</style>
