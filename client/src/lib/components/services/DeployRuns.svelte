<!--
    Despliegues del servicio, de un vistazo:
    - Acciones: desplegar y volver a la versión anterior (rollback, con confirmación).
    - El último despliegue, destacado: estado, rama, cuándo, cuánto tardó y si su versión es la
      activa. Si falló, un aviso sobrio que lleva a su salida (no el texto del error en rojo).
    - Los anteriores, en una lista compacta y atenuada: una línea cada uno, con el estado como
      punto de color y su etiqueta (nunca solo el color).
    - La salida del despliegue (snippet `output`) va plegada: se abre sola mientras se despliega
      y, después, a petición.
-->
<script lang="ts">
    import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
    import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
    import RocketIcon from '@lucide/svelte/icons/rocket';
    import Undo2Icon from '@lucide/svelte/icons/undo-2';
    import type { Snippet } from 'svelte';

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
        /** El servicio está en marcha (`false`: detenido; `null`: se desconoce). */
        serviceRunning?: boolean | null;
        /** Salida del último despliegue (p. ej. un visor de logs). */
        output?: Snippet;
        /** Para fijar «ahora» en tests e historias. */
        now?: number;
    }

    let {
        runs,
        blocked,
        activeRelease = null,
        previousRelease = null,
        onDeploy,
        onRollback,
        serviceRunning = null,
        output,
        now,
    }: Props = $props();

    const HISTORY = 6;
    const outputId = $props.id();
    let confirmRollback = $state(false);
    let showOutput = $state(false);

    const latest = $derived(runs?.[0] ?? null);
    const previous = $derived(runs?.slice(1, HISTORY) ?? []);
    const running = $derived(latest?.status === 'running');
    // Mientras se despliega, la salida se ve sin pedirla.
    const outputOpen = $derived(running || showOutput);

    const badge = {
        running: 'info',
        succeeded: 'success',
        failed: 'danger',
    } as const;

    const releaseDate = (release: string | null) => {
        const time = release ? releaseTime(release) : null;
        return time === null ? '' : formatDateTime(time);
    };
    const when = (run: DeployRun) => formatRelativeTime(run.startedAt * 1000, now);
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

    {#if latest}
        <section class="latest" aria-label={t('services.deploys.latest')}>
            <div class="latest-row">
                <Badge variant={badge[latest.status]} dot>{t(`services.deploys.status.${latest.status}`)}</Badge>
                {#if latest.kind === 'rollback'}<Badge>{t('services.deploys.kindRollback')}</Badge>{/if}
                {#if latest.release === activeRelease && !running}
                    <Badge variant="accent">{t('services.deploys.active')}</Badge>
                {/if}
                {#if latest.branch}<code class="branch">{latest.branch}</code>{/if}
                <span class="muted">{when(latest)}</span>
                {#if duration(latest)}<span class="muted">· {duration(latest)}</span>{/if}
                {#if output && !running}
                    <span class="toggle">
                        <Button
                            size="sm"
                            variant="ghost"
                            aria-expanded={outputOpen}
                            aria-controls={outputId}
                            onclick={() => (showOutput = !showOutput)}
                        >
                            {outputOpen ? t('services.deploys.hideOutput') : t('services.deploys.showOutput')}
                            <span class="chevron" class:open={outputOpen}><ChevronDownIcon /></span>
                        </Button>
                    </span>
                {/if}
            </div>
            {#if latest.status === 'succeeded' && latest.release === activeRelease && serviceRunning === false}
                <p class="hint">{t('services.deploys.serviceStopped')}</p>
            {/if}
            {#if latest.status === 'failed'}
                <p class="failure">
                    <CircleAlertIcon />
                    <span>{latest.detail ?? t('services.deploys.failedHint')}</span>
                </p>
            {/if}
            {#if output && outputOpen}
                <div id={outputId} class="output">{@render output()}</div>
            {/if}
        </section>
    {/if}

    {#if previous.length > 0}
        <section class="history" aria-label={t('services.deploys.history')}>
            <h4 class="history-title">{t('services.deploys.history')}</h4>
            <ul class="list">
                {#each previous as run (run.id)}
                    <li class="run">
                        <span class="state {run.status}">
                            <span class="dot" aria-hidden="true"></span>
                            {t(`services.deploys.status.${run.status}`)}
                        </span>
                        {#if run.kind === 'rollback'}<span class="muted">{t('services.deploys.kindRollback')}</span
                            >{/if}
                        {#if run.branch}<code class="branch">{run.branch}</code>{/if}
                        <span class="muted">{when(run)}</span>
                        {#if run.release === activeRelease}
                            <span class="active">{t('services.deploys.active')}</span>
                        {/if}
                    </li>
                {/each}
            </ul>
        </section>
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
        gap: var(--space-5);
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

    .latest {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
        padding: var(--space-3) var(--space-4);
        background-color: var(--color-bg-subtle);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
    }

    .latest-row {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-2);
        font-size: var(--font-size-sm);
    }

    .toggle {
        margin-left: auto;
    }

    .chevron {
        display: inline-flex;
        transition: transform var(--duration-fast) var(--easing-standard);
    }

    .chevron.open {
        transform: rotate(180deg);
    }

    .hint {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .failure {
        display: flex;
        gap: var(--space-2);
        align-items: flex-start;
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .failure :global(svg) {
        flex-shrink: 0;
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
        margin-top: var(--space-px);
        color: var(--color-danger);
    }

    .history {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
    }

    .history-title {
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-medium);
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
        gap: var(--space-3);
        padding: var(--space-1) var(--space-0);
        font-size: var(--font-size-sm);
    }

    .state {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
        min-width: var(--deploy-state-width);
        color: var(--color-text-muted);
    }

    .dot {
        width: var(--space-2);
        height: var(--space-2);
        background-color: var(--color-text-subtle);
        border-radius: var(--radius-full);
    }

    .succeeded .dot {
        background-color: var(--color-success);
    }

    .failed .dot {
        background-color: var(--color-danger);
    }

    .running .dot {
        background-color: var(--color-info);
    }

    .branch {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .muted {
        color: var(--color-text-muted);
    }

    .active {
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-medium);
        color: var(--color-accent);
    }
</style>
