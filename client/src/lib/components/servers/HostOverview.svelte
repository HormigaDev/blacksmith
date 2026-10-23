<!--
    El servidor de un vistazo: sistema y equipo, CPU (con su carga), memoria, swap, tarjetas
    gráficas, discos y red. Solo muestra lo que el agente pudo averiguar: lo desconocido se omite
    o se dice, nunca se inventa.
-->
<script lang="ts">
    import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
    import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
    import HardDriveIcon from '@lucide/svelte/icons/hard-drive';
    import NetworkIcon from '@lucide/svelte/icons/network';

    import type { HostInfo } from '$lib/agent';
    import { formatBytes, formatDateTime, formatDuration, formatNumber, t } from '$lib/i18n';

    import Badge from '../ui/Badge.svelte';
    import Meter from '../ui/Meter.svelte';
    import StatTile from '../ui/StatTile.svelte';

    interface Props {
        host: HostInfo;
    }

    let { host }: Props = $props();
    const id = $props.id();

    const unknown = t('host.unknown');
    const memoryUsed = $derived(Math.max(0, host.memory.totalBytes - host.memory.availableBytes));
    const swapUsed = $derived(Math.max(0, host.memory.swapTotalBytes - host.memory.swapFreeBytes));
    const load = $derived(host.loadAverage.map((value) => formatNumber(value, { maximumFractionDigits: 2 })));
    const threads = $derived(t('host.threads', { threads: formatNumber(host.cpu.logicalCores) }));
    const cpuDetail = $derived(
        [
            host.cpu.physicalCores && host.cpu.physicalCores !== host.cpu.logicalCores
                ? t('host.cores', { cores: formatNumber(host.cpu.physicalCores) })
                : null,
            host.cpu.frequencyMhz
                ? t('host.frequency', {
                      value: formatNumber(host.cpu.frequencyMhz / 1000, { maximumFractionDigits: 1 }),
                  })
                : null,
        ]
            .filter(Boolean)
            .join(' · ') || undefined,
    );
    const kernel = $derived(host.os.kernel ? `${host.os.kernel} (${host.os.architecture})` : host.os.architecture);
    const gpu = $derived(host.gpus[0] ?? null);
    const gpuDetail = $derived(
        gpu
            ? [
                  gpu.vendor,
                  gpu.driver ? t('host.gpuDriver', { driver: gpu.driver }) : null,
                  gpu.memoryBytes ? formatBytes(gpu.memoryBytes) : null,
              ]
                  .filter(Boolean)
                  .join(' · ')
            : t('host.noGpuDetail'),
    );
    const kindLabel = (kind: HostInfo['disks'][number]['kind']) => t(`host.diskKinds.${kind}`);
</script>

<div class="overview">
    <dl class="facts">
        <div class="fact">
            <dt>{t('host.os')}</dt>
            <dd>{host.os.name ?? host.os.id}</dd>
        </div>
        <div class="fact">
            <dt>{t('host.kernel')}</dt>
            <dd class="mono">{kernel}</dd>
        </div>
        <div class="fact">
            <dt>{t('host.processor')}</dt>
            <dd>{host.cpu.model ?? unknown}</dd>
        </div>
        <div class="fact">
            <dt>{t('host.machine')}</dt>
            <dd class="with-badge">
                <span>{host.machine ?? unknown}</span>
                {#if host.virtualization}
                    <Badge>{t('host.virtualized', { kind: host.virtualization.toUpperCase() })}</Badge>
                {/if}
            </dd>
        </div>
        {#if host.hostname}
            <div class="fact">
                <dt>{t('host.hostname')}</dt>
                <dd class="mono">{host.hostname}</dd>
            </div>
        {/if}
        <div class="fact">
            <dt>{t('host.uptime')}</dt>
            <dd title={formatDateTime(host.bootTime * 1000)}>{formatDuration(host.uptimeSecs)}</dd>
        </div>
        {#if host.timezone}
            <div class="fact">
                <dt>{t('host.timezone')}</dt>
                <dd>{host.timezone}</dd>
            </div>
        {/if}
        <div class="fact">
            <dt>{t('host.agent')}</dt>
            <dd class="mono">{t('host.agentVersion', { version: host.agentVersion })}</dd>
        </div>
    </dl>

    <div class="tiles">
        <StatTile label={t('host.cpu')} value={threads} detail={cpuDetail}>
            <div class="stack">
                <Meter
                    label={t('host.loadMeter', { threads: formatNumber(host.cpu.logicalCores) })}
                    value={host.loadAverage[0]}
                    max={host.cpu.logicalCores}
                    valueText={t('host.loadValue', { one: load[0], five: load[1], fifteen: load[2] })}
                />
                <p class="caption">{t('host.loadValue', { one: load[0], five: load[1], fifteen: load[2] })}</p>
            </div>
        </StatTile>

        <StatTile
            label={t('host.memory')}
            value={formatBytes(memoryUsed)}
            detail={t('host.memoryDetail', {
                available: formatBytes(host.memory.availableBytes),
                total: formatBytes(host.memory.totalBytes),
            })}
        >
            <Meter
                label={t('host.memoryMeter')}
                value={memoryUsed}
                max={host.memory.totalBytes}
                valueText={`${formatBytes(memoryUsed)} / ${formatBytes(host.memory.totalBytes)}`}
            />
        </StatTile>

        {#if host.memory.swapTotalBytes > 0}
            <StatTile
                label={t('host.swap')}
                value={formatBytes(swapUsed)}
                detail={t('host.swapDetail', { total: formatBytes(host.memory.swapTotalBytes) })}
            >
                <Meter
                    label={t('host.swapMeter')}
                    value={swapUsed}
                    max={host.memory.swapTotalBytes}
                    valueText={`${formatBytes(swapUsed)} / ${formatBytes(host.memory.swapTotalBytes)}`}
                />
            </StatTile>
        {:else}
            <StatTile label={t('host.swap')} kind="text" value={t('host.noSwap')} detail={t('host.noSwapDetail')} />
        {/if}

        <StatTile label={t('host.gpu')} kind="text" value={gpu?.name ?? t('host.noGpu')} detail={gpuDetail}>
            {#if host.gpus.length > 1}
                <ul class="more">
                    {#each host.gpus.slice(1) as other (other.address)}
                        <li>{other.name}</li>
                    {/each}
                </ul>
            {/if}
        </StatTile>
    </div>

    {#if host.disks.length > 0}
        <section class="group" aria-labelledby="{id}-disks">
            <h3 class="group-title" id="{id}-disks"><HardDriveIcon />{t('host.disks')}</h3>
            <ul class="list">
                {#each host.disks as disk (disk.mountPoint)}
                    {@const used = Math.max(0, disk.totalBytes - disk.availableBytes)}
                    <li class="item">
                        <div class="item-head">
                            <code class="name">{disk.mountPoint}</code>
                            <span class="muted">
                                {disk.device} · {disk.fileSystem}{disk.kind !== 'unknown'
                                    ? ` · ${kindLabel(disk.kind)}`
                                    : ''}
                            </span>
                            {#if disk.readOnly}<Badge>{t('host.readOnly')}</Badge>{/if}
                            <span class="end muted">
                                {t('host.diskFree', {
                                    free: formatBytes(disk.availableBytes),
                                    total: formatBytes(disk.totalBytes),
                                })}
                            </span>
                        </div>
                        <Meter
                            label={t('host.diskMeter', { mount: disk.mountPoint })}
                            value={used}
                            max={disk.totalBytes}
                            valueText={`${formatBytes(used)} / ${formatBytes(disk.totalBytes)}`}
                        />
                    </li>
                {/each}
            </ul>
        </section>
    {/if}

    {#if host.network.length > 0}
        <section class="group" aria-labelledby="{id}-network">
            <h3 class="group-title" id="{id}-network"><NetworkIcon />{t('host.network')}</h3>
            <ul class="list">
                {#each host.network as network (network.name)}
                    <li class="item item-row">
                        <code class="name">{network.name}</code>
                        <span class="addresses">
                            {#each network.addresses as address (address)}<code class="address">{address}</code>{/each}
                        </span>
                        <span class="end traffic">
                            <span class="flow" title={t('host.received')}>
                                <ArrowDownIcon aria-hidden="true" />
                                <span class="bs-visually-hidden">{t('host.received')}</span>
                                {formatBytes(network.receivedBytes)}
                            </span>
                            <span class="flow" title={t('host.transmitted')}>
                                <ArrowUpIcon aria-hidden="true" />
                                <span class="bs-visually-hidden">{t('host.transmitted')}</span>
                                {formatBytes(network.transmittedBytes)}
                            </span>
                        </span>
                    </li>
                {/each}
            </ul>
        </section>
    {/if}
</div>

<style>
    .overview {
        display: flex;
        flex-direction: column;
        gap: var(--space-6);
    }

    .facts {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(min(100%, var(--layout-tile-min-width)), 1fr));
        gap: var(--space-4);
        margin: var(--space-0);
    }

    .fact {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
        min-width: 0;
    }

    dt {
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-medium);
        color: var(--color-text-muted);
    }

    dd {
        margin: var(--space-0);
        font-size: var(--font-size-sm);
        color: var(--color-text);
        overflow-wrap: anywhere;
    }

    .mono {
        font-family: var(--font-mono);
    }

    .with-badge {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-2);
        align-items: center;
    }

    .tiles {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(min(100%, var(--layout-tile-min-width)), 1fr));
        gap: var(--space-3);
    }

    .stack {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
    }

    .caption {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .more {
        padding: var(--space-0);
        margin: var(--space-0);
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
        list-style: none;
    }

    .group {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .group-title {
        display: flex;
        gap: var(--space-2);
        align-items: center;
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-semibold);
    }

    .group-title :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
        color: var(--color-accent);
    }

    .list {
        display: flex;
        flex-direction: column;
        padding: var(--space-0);
        margin: var(--space-0);
        list-style: none;
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
    }

    .item {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        padding: var(--space-3) var(--space-4);
        font-size: var(--font-size-sm);
    }

    .item + .item {
        border-top: var(--border-width) solid var(--color-border);
    }

    .item-row {
        flex-direction: row;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-2) var(--space-4);
    }

    .item-head {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-2) var(--space-3);
        align-items: center;
    }

    .name {
        font-weight: var(--font-weight-medium);
    }

    .muted {
        color: var(--color-text-muted);
    }

    .end {
        margin-left: auto;
    }

    .addresses {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-2);
        min-width: 0;
    }

    .address {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
        overflow-wrap: anywhere;
    }

    .traffic {
        display: flex;
        gap: var(--space-3);
        color: var(--color-text-muted);
    }

    .flow {
        display: inline-flex;
        gap: var(--space-1);
        align-items: center;
    }

    .flow :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
    }
</style>
