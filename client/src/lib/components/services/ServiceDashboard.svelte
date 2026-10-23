<!--
    Resumen del servicio de un vistazo: CPU y memoria con su tendencia de los últimos minutos,
    memoria frente a su límite (o a la RAM del servidor), swap, procesos (con sus límites, si los
    tiene), tiempo en marcha,
    reinicios tras fallos, la versión activa y su web.

    Todo sale del cgroup del servicio (systemd): suma todos sus procesos, no solo el principal.
    La tendencia la acumula la interfaz con cada lectura de estado (`history`).
-->
<script lang="ts">
    import { releaseTime, type ServiceStatus, type Site } from '$lib/agent';
    import { formatBytes, formatDateTime, formatDuration, formatNumber, formatPercent, t } from '$lib/i18n';
    import type { ResourceSample } from '$lib/services/resource-history';

    import Meter from '../ui/Meter.svelte';
    import Sparkline from '../ui/Sparkline.svelte';
    import StatTile from '../ui/StatTile.svelte';

    interface Props {
        /** `null` mientras carga. */
        status: ServiceStatus | null;
        history: readonly ResourceSample[];
        /** Rama de la versión activa (de su despliegue), si se conoce. */
        branch?: string | null;
        /** `undefined` mientras carga; `null` sin sitio. */
        site?: Site | null;
        /** Para fijar «ahora» en tests e historias. */
        now?: number;
    }

    let { status, history, branch = null, site, now }: Props = $props();

    const resources = $derived(status?.resources ?? null);
    const host = $derived(status?.host ?? null);
    // Los mismos estados en los que el agente mide el consumo (services::status).
    const running = $derived(status !== null && ['active', 'reloading', 'deactivating'].includes(status.activeState));
    const unknown = t('services.dashboard.unknown');

    const cpu = (value: number) => formatPercent(value / 100);
    const bytes = (value: number) => formatBytes(value);

    const memoryCap = $derived(resources?.memoryLimitBytes ?? host?.memoryBytes ?? null);
    const memoryCapLabel = $derived(
        resources?.memoryLimitBytes
            ? t('services.dashboard.memoryOfLimit', { limit: bytes(resources.memoryLimitBytes) })
            : t('services.dashboard.memoryOfHost', { total: bytes(host?.memoryBytes ?? 0) }),
    );

    const swapDetail = $derived.by(() => {
        if (!host || host.swapBytes === 0) return t('services.dashboard.noSwap');
        const limit = resources?.swapLimitBytes;
        if (limit === 0) return t('services.dashboard.swapDisabled');
        if (limit != null && limit < host.swapBytes) {
            return t('services.dashboard.swapLimited', { limit: bytes(limit), total: bytes(host.swapBytes) });
        }
        return t('services.dashboard.swapDetail', { total: bytes(host.swapBytes) });
    });
    const since = $derived(resources?.uptimeSecs != null ? (now ?? Date.now()) - resources.uptimeSecs * 1000 : null);
    const releaseAt = $derived(status?.release ? releaseTime(status.release) : null);
    // Sin ninguna lectura (p. ej. parado desde que se abrió), no se dibuja una tendencia vacía.
    const hasCpu = $derived(history.some((sample) => sample.cpu !== null));
    const hasMemory = $derived(history.some((sample) => sample.memory !== null));
    const siteUrl = $derived(site ? `${site.tls === 'active' ? 'https' : 'http'}://${site.domains[0] ?? ''}` : null);
</script>

<div class="dashboard">
    {#if status && !running}
        <p class="note" role="status">{t('services.dashboard.stopped')}</p>
    {/if}

    <div class="tiles">
        <StatTile
            label={t('services.dashboard.cpu')}
            value={resources?.cpuPercent != null ? cpu(resources.cpuPercent) : unknown}
            detail={resources?.cpuLimitPercent != null && host
                ? t('services.dashboard.cpuLimited', {
                      limit: cpu(resources.cpuLimitPercent),
                      cores: formatNumber(host.cpuCores),
                  })
                : host
                  ? t('services.dashboard.cpuDetail', { cores: formatNumber(host.cpuCores) })
                  : undefined}
        >
            {#if hasCpu}
                <Sparkline
                    label={t('services.dashboard.cpuTrend')}
                    values={history.map((sample) => sample.cpu)}
                    times={history.map((sample) => sample.at)}
                    format={cpu}
                    max={resources?.cpuLimitPercent ?? undefined}
                />
            {/if}
        </StatTile>

        <StatTile
            label={t('services.dashboard.memory')}
            value={resources?.memoryBytes != null ? bytes(resources.memoryBytes) : unknown}
            detail={t('services.dashboard.memoryDetail')}
        >
            <div class="stack">
                {#if resources?.memoryBytes != null && memoryCap}
                    <Meter
                        label={memoryCapLabel}
                        value={resources.memoryBytes}
                        max={memoryCap}
                        valueText={`${bytes(resources.memoryBytes)} / ${bytes(memoryCap)}`}
                    />
                {/if}
                {#if hasMemory}
                    <Sparkline
                        label={t('services.dashboard.memoryTrend')}
                        values={history.map((sample) => sample.memory)}
                        times={history.map((sample) => sample.at)}
                        format={bytes}
                        max={resources?.memoryLimitBytes ?? undefined}
                    />
                {/if}
            </div>
        </StatTile>

        <StatTile
            label={t('services.dashboard.swap')}
            value={resources?.swapBytes != null ? bytes(resources.swapBytes) : unknown}
            detail={swapDetail}
        />

        <StatTile
            label={t('services.dashboard.tasks')}
            value={resources?.tasks != null ? formatNumber(resources.tasks) : unknown}
            detail={resources?.tasksLimit != null
                ? t('services.dashboard.tasksLimited', { limit: formatNumber(resources.tasksLimit) })
                : t('services.dashboard.tasksDetail')}
        />

        <StatTile
            label={t('services.dashboard.uptime')}
            value={resources?.uptimeSecs != null ? formatDuration(resources.uptimeSecs) : unknown}
            detail={since !== null ? t('services.dashboard.uptimeDetail', { since: formatDateTime(since) }) : undefined}
        />

        <StatTile
            label={t('services.dashboard.restarts')}
            value={resources?.restarts != null ? formatNumber(resources.restarts) : unknown}
            detail={resources?.restarts ? t('services.dashboard.restartsSome') : t('services.dashboard.restartsNone')}
        />

        <StatTile
            label={t('services.dashboard.release')}
            kind="text"
            value={releaseAt !== null
                ? t('services.dashboard.releaseValue', { date: formatDateTime(releaseAt) })
                : t('services.dashboard.noRelease')}
            detail={branch ? t('services.dashboard.releaseBranch', { branch }) : undefined}
        />

        {#if site !== undefined}
            <StatTile
                label={t('services.dashboard.site')}
                kind="text"
                value={siteUrl ?? t('services.dashboard.noSite')}
                detail={site ? t(`services.site.tls.${site.tls}`) : undefined}
            />
        {/if}
    </div>

    <p class="footnote">{t('services.dashboard.accounting')}</p>
</div>

<style>
    .dashboard {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
    }

    .tiles {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(min(100%, var(--layout-tile-min-width)), 1fr));
        gap: var(--space-3);
    }

    .stack {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .note {
        padding: var(--space-3);
        font-size: var(--font-size-sm);
        background-color: var(--color-bg-subtle);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
    }

    .footnote {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }
</style>
