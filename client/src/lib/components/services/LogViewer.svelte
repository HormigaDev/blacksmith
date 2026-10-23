<!--
    Visor de logs en tiempo real. Solo presenta: el flujo lo lleva `LogSession`
    ($lib/services/log-session.svelte).

    - Se desplaza solo al final mientras el usuario esté abajo; si sube a leer, no lo arrastra.
    - Los códigos de color ANSI se quitan al mostrar; cada línea se tiñe según su prioridad.
-->
<script lang="ts">
    import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
    import { tick } from 'svelte';

    import type { LogEndReason } from '$lib/agent';
    import { formatDateTime, t } from '$lib/i18n';
    import type { LogLine, LogStatus } from '$lib/services/log-session.svelte';

    import Badge from '../ui/Badge.svelte';
    import Button from '../ui/Button.svelte';

    interface Props {
        /** Nombre del servicio, para el nombre accesible del visor. */
        name: string;
        lines: readonly LogLine[];
        status: LogStatus;
        endReason?: LogEndReason | null;
        error?: string | null;
        onReconnect?: () => void;
    }

    let { name, lines, status, endReason = null, error = null, onReconnect }: Props = $props();

    /** Distancia al final (px) por debajo de la cual se sigue considerando «abajo». */
    const STICK_THRESHOLD = 24;
    // eslint-disable-next-line no-control-regex
    const ANSI = /\u001b\[[0-9;?]*[ -/]*[@-~]/g;

    let viewport = $state<HTMLElement>();
    let stick = true;

    const badge = $derived(
        (
            {
                idle: 'neutral',
                connecting: 'info',
                live: 'success',
                ended: 'neutral',
                error: 'danger',
            } as const
        )[status],
    );
    const notice = $derived(
        status === 'error' ? error : status === 'ended' && endReason ? t(`services.logs.ended.${endReason}`) : null,
    );

    function onscroll(): void {
        if (!viewport) return;
        stick = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight < STICK_THRESHOLD;
    }

    $effect(() => {
        void lines.length;
        if (!stick || !viewport) return;
        const element = viewport;
        void tick().then(() => (element.scrollTop = element.scrollHeight));
    });

    const time = (timestampUs: number) =>
        formatDateTime(timestampUs / 1000, {
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit',
            fractionalSecondDigits: 3,
            hour12: false,
        });

    function level(priority: number | null): 'error' | 'warning' | 'debug' | 'info' {
        if (priority === null) return 'info';
        if (priority <= 3) return 'error';
        if (priority === 4) return 'warning';
        if (priority === 7) return 'debug';
        return 'info';
    }
</script>

<section class="viewer">
    <header class="toolbar">
        <span aria-live="polite"><Badge variant={badge} dot>{t(`services.logs.status.${status}`)}</Badge></span>
        {#if notice}<p class="notice" role={status === 'error' ? 'alert' : undefined}>{notice}</p>{/if}
        {#if onReconnect && (status === 'ended' || status === 'error' || status === 'idle')}
            <Button size="sm" onclick={onReconnect}>
                <RefreshCwIcon />
                {t('services.logs.reconnect')}
            </Button>
        {/if}
    </header>

    <!--
        `aria-live="off"`: una ráfaga de cien líneas no debe leerse entera.
        `tabindex`: un área con scroll debe poder recorrerse con el teclado (WCAG 2.1.1, regla
        `scrollable-region-focusable` de axe), aunque `log` no sea un rol interactivo.
    -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
        class="lines"
        data-selectable
        role="log"
        aria-live="off"
        aria-label={t('services.logs.label', { name })}
        tabindex="0"
        bind:this={viewport}
        {onscroll}
    >
        {#each lines as line (line.id)}
            <div class="line {level(line.priority)}">
                <span class="time">{time(line.timestampUs)}</span>
                <span class="message">{line.message.replace(ANSI, '')}</span>
            </div>
        {:else}
            {#if status === 'live'}<p class="empty">{t('services.logs.empty')}</p>{/if}
        {/each}
    </div>
</section>

<style>
    .viewer {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
    }

    .toolbar {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-3);
    }

    .notice {
        flex: 1;
        min-width: 0;
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .lines {
        height: var(--log-viewer-height);
        padding: var(--space-3);
        overflow: auto;
        font-family: var(--font-mono);
        font-size: var(--font-size-xs);
        line-height: var(--line-height-normal);
        background-color: var(--color-bg-subtle);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
    }

    .lines:focus-visible {
        outline: var(--focus-ring-width) solid var(--color-focus-ring);
        outline-offset: var(--focus-ring-width);
    }

    .line {
        display: flex;
        gap: var(--space-3);
        color: var(--color-text);
    }

    .time {
        flex-shrink: 0;
        color: var(--color-text-subtle);
    }

    .message {
        min-width: 0;
        overflow-wrap: anywhere;
        white-space: pre-wrap;
    }

    .error .message {
        color: var(--color-danger);
    }

    .warning .message {
        color: var(--color-warning);
    }

    .debug .message {
        color: var(--color-text-muted);
    }

    .empty {
        color: var(--color-text-muted);
    }
</style>
