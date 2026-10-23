<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'LogViewer',
        group: 'Servicios',
        description: 'Logs de un servicio en tiempo real.',
    };
</script>

<script lang="ts">
    import type { LogLine } from '$lib/services/log-session.svelte';
    import { Story } from '$lib/workbench';

    import LogViewer from '../LogViewer.svelte';

    // Hora fija: las capturas visuales no deben depender del momento.
    const at = (second: number) => Date.UTC(2026, 9, 22, 12, 0, second, 250) * 1000;
    const lines: LogLine[] = [
        {
            id: 1,
            timestampUs: at(0),
            priority: 6,
            pid: 812,
            identifier: 'api',
            message: 'Escuchando en 127.0.0.1:8080',
        },
        { id: 2, timestampUs: at(1), priority: 7, pid: 812, identifier: 'api', message: 'GET /health 200 (2 ms)' },
        {
            id: 3,
            timestampUs: at(3),
            priority: 4,
            pid: 812,
            identifier: 'api',
            message: 'Reintentando conexión con la base de datos (1/3)',
        },
        {
            id: 4,
            timestampUs: at(5),
            priority: 3,
            pid: 812,
            identifier: 'api',
            message: 'Error: ECONNREFUSED 127.0.0.1:5432',
        },
        {
            id: 5,
            timestampUs: at(6),
            priority: null,
            pid: null,
            identifier: null,
            message:
                'Línea sin prioridad, muy larga para comprobar que se parte en varias líneas sin desbordar el visor ni romper la columna de la hora.',
        },
    ];
    const noop = () => {};
</script>

<Story name="En vivo" layout="stack">
    <div class="frame"><LogViewer name="API" status="live" {lines} /></div>
</Story>

<Story name="Sin entradas" layout="stack">
    <div class="frame"><LogViewer name="API" status="live" lines={[]} /></div>
</Story>

<Story name="Conexión perdida" layout="stack">
    <div class="frame">
        <LogViewer name="API" status="ended" endReason="connectionLost" {lines} onReconnect={noop} />
    </div>
</Story>

<Story name="Error" layout="stack">
    <div class="frame">
        <LogViewer
            name="API"
            status="error"
            error="Hay demasiados visores de logs abiertos en este servidor. Cierra alguno e inténtalo de nuevo."
            lines={[]}
            onReconnect={noop}
        />
    </div>
</Story>

<style>
    .frame {
        width: min(100%, var(--layout-content-max-width));
    }
</style>
