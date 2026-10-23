<!--
    Despliegues del servicio: lanzar uno, volver a la versión anterior (rollback, con
    confirmación) y los últimos con su resultado. La versión activa va marcada.
-->
<script lang="ts">
    import RocketIcon from '@lucide/svelte/icons/rocket';
    import Undo2Icon from '@lucide/svelte/icons/undo-2';

    import { type DeployRun, releaseTime } from '$lib/agent';
    import { formatDateTime, formatNumber, formatRelativeTime, t } from '$lib/i18n';

    import Badge from '../ui/Badge.svelte';
    import Button from '../ui/Button.svelte';
    import ConfirmDialog from '../ui/ConfirmDialog.svelte';

    interface Props {
        runs: readonly DeployRun[] | null;
        /** Por qué no se puede desplegar aún (`null` si se puede). */
        blocked: string | null;
        /** Versión activa y la anterior (`null` si no hay). */
        activeRelease?: string | null;
        previousRelease?: string | null;
        onDeploy: () => void | Promise<void>;
        onRollback?: () => Promise<void>;
        /** Para fijar «ahora» en tests e historias. */
        now?: number;
    }

    let { runs, blocked, activeRelease = null, previousRelease = null, onDeploy, onRollback, now }: Props = $props();

    let confirmRollback = $state(false);
    const releaseDate = (release: string | null) => {
        const time = release ? releaseTime(release) : null;
        return time === null ? '' : formatDateTime(time);
    };

    const running = $derived(runs?.[0]?.status === 'running');
    const badge = {
        running: 'info',
        succeeded: 'success',
        failed: 'danger',
    } as const;

    const duration = (run: DeployRun) =>
        run.finishedAt === null
            ? null
            : t('services.deploys.duration', {
                  seconds: formatNumber(Math.max(0, run.finishedAt - run.startedAt)),
              });
</script>

<div class="runs">
    <div class="actions">
        <Button variant="primary" disabled={blocked !== null || running} loading={running} onclick={onDeploy}>
            <RocketIcon />
            {running ? t('services.deploys.running') : t('services.deploys.deploy')}
        </Button>
        {#if previousRelease && onRollback}
            <Button disabled={running} onclick={() => (confirmRollback = true)}>
                <Undo2Icon />
                {t('services.deploys.rollback')}
            </Button>
        {/if}
        {#if blocked}<span class="note">{blocked}</span>{/if}
    </div>

    {#if runs && runs.length > 0}
        <ul class="list">
            {#each runs.slice(0, 5) as run (run.id)}
                <li class="run">
                    <Badge variant={badge[run.status]} dot>{t(`services.deploys.status.${run.status}`)}</Badge>
                    {#if run.kind === 'rollback'}<Badge>{t('services.deploys.kindRollback')}</Badge>{/if}
                    {#if run.release === activeRelease && run.status !== 'running'}
                        <Badge variant="accent">{t('services.deploys.active')}</Badge>
                    {/if}
                    {#if run.branch}<code class="branch">{run.branch}</code>{/if}
                    <span class="when">{formatRelativeTime(run.startedAt * 1000, now)}</span>
                    {#if duration(run)}<span class="when">· {duration(run)}</span>{/if}
                    {#if run.detail}<span class="detail">{run.detail}</span>{/if}
                </li>
            {/each}
        </ul>
    {/if}
</div>

{#if onRollback}
    <ConfirmDialog
        bind:open={confirmRollback}
        title={t('services.deploys.rollbackTitle')}
        description={t('services.deploys.rollbackDescription', { date: releaseDate(previousRelease) })}
        confirmLabel={t('services.deploys.rollbackConfirm')}
        onConfirm={onRollback}
    />
{/if}

<style>
    .runs {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
    }

    .actions {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-3);
    }

    .note {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .list {
        display: flex;
        flex-direction: column;
        padding: var(--space-0);
        margin: var(--space-0);
        list-style: none;
    }

    .run {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-2) var(--space-0);
        font-size: var(--font-size-sm);
        border-bottom: var(--border-width) solid var(--color-border);
    }

    .run:last-child {
        border-bottom: none;
    }

    .branch {
        font-size: var(--font-size-xs);
    }

    .when {
        color: var(--color-text-muted);
    }

    .detail {
        flex-basis: 100%;
        font-size: var(--font-size-xs);
        color: var(--color-danger);
    }
</style>
