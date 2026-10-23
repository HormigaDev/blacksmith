<!--
    Estado del servicio y sus acciones: iniciar/detener (un único botón según el estado) y
    reiniciar (solo con el servicio en marcha). Todo queda deshabilitado mientras la unidad no
    exista en el servidor. Detener y reiniciar cortan el servicio: se confirman antes.

    Un sitio solo estático (sin comando de arranque) no tiene proceso: lo sirve nginx. Entonces
    solo se indica, sin acciones.
-->
<script lang="ts">
    import PlayIcon from '@lucide/svelte/icons/play';
    import RotateCwIcon from '@lucide/svelte/icons/rotate-cw';
    import SquareIcon from '@lucide/svelte/icons/square';

    import type { ServiceAction, ServiceStatus } from '$lib/agent';
    import { formatBytes, formatNumber, t } from '$lib/i18n';

    import Badge from '../ui/Badge.svelte';
    import Button from '../ui/Button.svelte';
    import ConfirmDialog from '../ui/ConfirmDialog.svelte';

    interface Props {
        /** Nombre visible, para las confirmaciones. */
        name: string;
        /** `null` mientras se desconoce (cargando o sin conexión). */
        status: ServiceStatus | null;
        /** Acción en curso. */
        busy?: ServiceAction | null;
        /** Sin comando de arranque: lo sirve nginx y no hay proceso que controlar. */
        staticOnly?: boolean;
        onAction: (action: ServiceAction) => void | Promise<void>;
    }

    let { name, status, busy = null, staticOnly = false, onAction }: Props = $props();

    let confirm = $state<'stop' | 'restart' | null>(null);

    const deployed = $derived(status?.loadState === 'loaded' && status.deployed);
    const running = $derived(
        status !== null && ['active', 'reloading', 'activating', 'refreshing'].includes(status.activeState),
    );
    const badge = $derived.by(() => {
        if (!status) return { variant: 'neutral', label: t('services.controls.unknown') } as const;
        const variant = (
            {
                active: 'success',
                reloading: 'info',
                activating: 'info',
                refreshing: 'info',
                deactivating: 'warning',
                maintenance: 'warning',
                failed: 'danger',
                inactive: 'neutral',
            } as const
        )[status.activeState];
        return { variant, label: t(`services.controls.state.${status.activeState}`) };
    });
    const metrics = $derived(
        status?.metrics
            ? t('services.controls.metrics', {
                  cpu: `${formatNumber(status.metrics.cpuPercent, { maximumFractionDigits: 1 })} %`,
                  ram: formatBytes(status.metrics.rssBytes),
              })
            : null,
    );
    const confirmOpen = {
        get value() {
            return confirm !== null;
        },
        set value(open: boolean) {
            if (!open) confirm = null;
        },
    };
</script>

<section class="controls" aria-label={t('services.controls.label')}>
    {#if staticOnly}
        <div class="state">
            <Badge variant="info">{t('services.controls.staticSite')}</Badge>
            <span class="hint">
                {status?.deployed ? t('services.controls.staticServed') : t('services.controls.notDeployed')}
            </span>
        </div>
    {:else}
        <div class="state">
            <span aria-live="polite"><Badge variant={badge.variant} dot>{badge.label}</Badge></span>
            {#if metrics}<span class="metrics">{metrics}</span>{/if}
            {#if status && !deployed}<span class="hint">{t('services.controls.notDeployed')}</span>{/if}
        </div>
        <div class="actions">
            {#if running}
                <Button
                    size="sm"
                    disabled={!deployed || busy !== null}
                    loading={busy === 'stop'}
                    onclick={() => (confirm = 'stop')}
                >
                    <SquareIcon />
                    {t('services.controls.stop')}
                </Button>
            {:else}
                <Button
                    size="sm"
                    variant="primary"
                    disabled={!deployed || busy !== null}
                    loading={busy === 'start'}
                    onclick={() => onAction('start')}
                >
                    <PlayIcon />
                    {t('services.controls.start')}
                </Button>
            {/if}
            <Button
                size="sm"
                disabled={!deployed || !running || busy !== null}
                loading={busy === 'restart'}
                onclick={() => (confirm = 'restart')}
            >
                <RotateCwIcon />
                {t('services.controls.restart')}
            </Button>
        </div>
    {/if}
</section>

<ConfirmDialog
    bind:open={confirmOpen.value}
    variant={confirm === 'stop' ? 'danger' : 'primary'}
    title={confirm === 'stop'
        ? t('services.controls.stopTitle', { name })
        : t('services.controls.restartTitle', { name })}
    description={confirm === 'stop'
        ? t('services.controls.stopDescription')
        : t('services.controls.restartDescription')}
    confirmLabel={confirm === 'stop' ? t('services.controls.stop') : t('services.controls.restart')}
    onConfirm={() => (confirm ? onAction(confirm) : undefined)}
/>

<style>
    .controls {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-3);
    }

    .state {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-3);
        font-size: var(--font-size-sm);
    }

    .metrics {
        font-family: var(--font-mono);
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .hint {
        color: var(--color-text-muted);
    }

    .actions {
        display: flex;
        gap: var(--space-2);
    }
</style>
