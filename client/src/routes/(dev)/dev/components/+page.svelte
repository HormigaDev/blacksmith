<!--
    Catálogo de componentes, solo en desarrollo: http://localhost:1420/dev/components
    Al ser una herramienta interna, sus textos no pasan por i18n (excluida en check:i18n).
-->
<script lang="ts">
    import LayoutGridIcon from '@lucide/svelte/icons/layout-grid';
    import ListIcon from '@lucide/svelte/icons/list';
    import RadioIcon from '@lucide/svelte/icons/radio';
    import SearchIcon from '@lucide/svelte/icons/search';
    import ServerIcon from '@lucide/svelte/icons/server';
    import Trash2Icon from '@lucide/svelte/icons/trash-2';

    import LocaleSwitcher from '$lib/components/locale/LocaleSwitcher.svelte';
    import ThemeSwitcher from '$lib/components/theme/ThemeSwitcher.svelte';
    import {
        Badge,
        Button,
        Checkbox,
        ConfirmDialog,
        DatePicker,
        Dialog,
        FilePicker,
        Input,
        Select,
        Spinner,
        Switch,
        Textarea,
        TimePicker,
        ToggleButton,
        ToggleGroup,
        Tooltip,
    } from '$lib/components/ui';
    import { formatBytes, formatDateTime, formatRelativeTime } from '$lib/i18n';
    import { toast } from '$lib/toast';

    let host = $state('');
    let password = $state('');
    let port = $state<number | null>(22);
    let search = $state('');
    let notes = $state('');
    let unit = $state<string>();
    let branch = $state<'main' | 'develop' | 'release'>('main');
    let date = $state<string>();
    let time = $state<string>('09:30');
    let keys = $state<string[]>([]);
    let folders = $state<string[]>([]);
    let autoDeploy = $state(true);
    let notify = $state(false);
    let accepted = $state(false);
    let partial = $state(true);
    let view = $state<'list' | 'grid'>('list');
    let range = $state<'1h' | '24h' | '7d'>('24h');
    let follow = $state(true);
    let dialogOpen = $state(false);
    let confirmOpen = $state(false);

    const units = [
        { value: 'nginx.service', label: 'nginx.service' },
        { value: 'api.service', label: 'api.service' },
        { value: 'worker.service', label: 'worker.service' },
        { value: 'legacy.service', label: 'legacy.service (deshabilitado)', disabled: true },
    ];
    const branches = [
        { value: 'main', label: 'main' },
        { value: 'develop', label: 'develop' },
        { value: 'release', label: 'release' },
    ] as const;

    const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
</script>

<svelte:head>
    <title>Componentes · dev</title>
</svelte:head>

<div class="page">
    <header class="top">
        <div>
            <h1>Componentes</h1>
            <p class="muted">Catálogo de src/lib/components/ui. Solo visible en desarrollo.</p>
        </div>
        <div class="row">
            <LocaleSwitcher />
            <ThemeSwitcher />
        </div>
    </header>

    <section class="card">
        <h2>Botones</h2>
        <div class="row">
            <Button variant="primary">Desplegar</Button>
            <Button>Secundario</Button>
            <Button variant="ghost">Ghost</Button>
            <Button variant="danger">Detener</Button>
            <Button variant="primary" loading>Guardando</Button>
            <Button disabled>Deshabilitado</Button>
            <Tooltip text="Eliminar servidor">
                {#snippet children(props)}
                    <Button {...props} iconOnly variant="ghost" aria-label="Eliminar servidor"><Trash2Icon /></Button>
                {/snippet}
            </Tooltip>
        </div>
        <div class="row">
            <Button size="sm">Pequeño</Button>
            <Button size="md">Mediano</Button>
            <Button size="lg" variant="primary">Grande</Button>
            <Spinner />
            <Spinner size="lg" label="Cargando" />
        </div>
    </section>

    <section class="card">
        <h2>Campos de texto</h2>
        <div class="grid">
            <Input
                label="Host"
                placeholder="vps.example.com"
                hint="Nombre o IP del servidor"
                required
                bind:value={host}
            >
                {#snippet trailing()}<ServerIcon />{/snippet}
            </Input>
            <Input label="Contraseña" type="password" bind:value={password} />
            <Input
                label="Puerto SSH"
                type="number"
                min={1}
                max={65535}
                hint="Enfoca para ver los controles"
                bind:value={port}
            />
            <Input label="Buscar" type="search" placeholder="Filtrar servicios…" bind:value={search}>
                {#snippet leading()}<SearchIcon />{/snippet}
            </Input>
            <Input label="Con error" value="no-es-un-host" error="El host no es válido" />
            <Input label="Deshabilitado" value="solo lectura" disabled />
        </div>
        <Textarea label="Notas del despliegue" placeholder="Escribe algo…" maxlength={280} bind:value={notes}>
            {#snippet trailing()}<RadioIcon />{/snippet}
        </Textarea>
        <p class="muted">host={host} · puerto={port} · notas={notes.length}</p>
    </section>

    <section class="card">
        <h2>Selección</h2>
        <div class="grid">
            <Select label="Unidad systemd" options={units} bind:value={unit} hint="Con typeahead y teclado" />
            <Select label="Rama" options={branches} bind:value={branch} />
            <Select label="Con error" options={units} error="Selecciona una unidad" />
            <Select label="Deshabilitado" options={units} disabled />
        </div>
        <p class="muted">unidad={unit ?? '—'} · rama={branch}</p>
    </section>

    <section class="card">
        <h2>Fecha y hora</h2>
        <div class="grid">
            <DatePicker label="Fecha de despliegue" bind:value={date} hint="Escribe o usa el calendario" />
            <DatePicker label="Con límites" min="2026-10-01" max="2026-10-31" />
            <TimePicker label="Hora" bind:value={time} />
            <TimePicker label="Con segundos (24 h)" granularity="second" hourCycle={24} />
        </div>
        <p class="muted">
            fecha={date ?? '—'} · hora={time ?? '—'} · ahora={formatDateTime(Date.now())} · hace={formatRelativeTime(
                Date.now() - 5 * 60_000,
            )} · {formatBytes(1_572_864)}
        </p>
    </section>

    <section class="card">
        <h2>Archivos</h2>
        <div class="grid">
            <FilePicker
                label="Llave SSH"
                accept={['pem', 'key', 'pub']}
                hint="En Tauri devuelve la ruta"
                bind:value={keys}
            />
            <FilePicker label="Directorios del proyecto" directory multiple bind:value={folders} />
        </div>
    </section>

    <section class="card">
        <h2>Interruptores y casillas</h2>
        <div class="grid">
            <div class="stack">
                <Switch
                    label="Despliegue automático"
                    description="Desplegar al hacer push a la rama"
                    bind:checked={autoDeploy}
                />
                <Switch label="Notificar fallos" bind:checked={notify} />
                <Switch label="Deshabilitado" disabled />
            </div>
            <div class="stack">
                <Checkbox label="Acepto reiniciar el servicio" bind:checked={accepted} />
                <Checkbox
                    label="Seleccionar todo"
                    description="Estado mixto"
                    bind:checked={partial}
                    bind:indeterminate={partial}
                />
                <Checkbox label="Deshabilitado" disabled checked />
            </div>
        </div>
        <div class="row">
            <ToggleGroup
                label="Vista"
                iconOnly
                options={[
                    { value: 'list', label: 'Lista', icon: ListIcon },
                    { value: 'grid', label: 'Cuadrícula', icon: LayoutGridIcon },
                ]}
                bind:value={view}
            />
            <ToggleGroup
                label="Rango"
                options={[
                    { value: '1h', label: '1 h' },
                    { value: '24h', label: '24 h' },
                    { value: '7d', label: '7 días' },
                ]}
                bind:value={range}
            />
            <ToggleButton bind:pressed={follow}><RadioIcon /> Seguir logs</ToggleButton>
        </div>
        <p class="muted">vista={view} · rango={range} · seguir={follow}</p>
    </section>

    <section class="card">
        <h2>Estados, diálogos y notificaciones</h2>
        <div class="row">
            <Badge>neutral</Badge>
            <Badge variant="accent">accent</Badge>
            <Badge variant="success" dot>activo</Badge>
            <Badge variant="warning" dot>reiniciando</Badge>
            <Badge variant="danger" dot>fallido</Badge>
            <Badge variant="info">info</Badge>
        </div>
        <div class="row">
            <Button onclick={() => (dialogOpen = true)}>Abrir diálogo</Button>
            <Button variant="danger" onclick={() => (confirmOpen = true)}>Detener servicio</Button>
            <Button
                onclick={() => toast.success('Despliegue completado', { description: 'api.service · main@3983fd9' })}
            >
                Toast éxito
            </Button>
            <Button onclick={() => toast.error('No se pudo conectar con el agente')}>Toast error</Button>
            <Button onclick={() => toast.warning('El servicio tardó en arrancar')}>Toast aviso</Button>
            <Button onclick={() => toast.info('Hay una nueva versión del agente')}>Toast info</Button>
        </div>
    </section>
</div>

<Dialog bind:open={dialogOpen} title="Nuevo servidor" description="Se conectará por túnel SSH.">
    <div class="stack">
        <Input label="Host" placeholder="vps.example.com" />
        <Input label="Usuario" placeholder="deploy" />
    </div>
    {#snippet footer()}
        <Button onclick={() => (dialogOpen = false)}>Cancelar</Button>
        <Button variant="primary" onclick={() => (dialogOpen = false)}>Guardar</Button>
    {/snippet}
</Dialog>

<ConfirmDialog
    bind:open={confirmOpen}
    variant="danger"
    title="¿Detener api.service?"
    description="El servicio dejará de atender peticiones hasta que vuelvas a iniciarlo."
    confirmLabel="Detener"
    onConfirm={async () => {
        await sleep(1200);
        toast.success('api.service detenido');
    }}
/>

<style>
    .page {
        display: flex;
        flex-direction: column;
        gap: var(--space-6);
        max-width: var(--layout-content-max-width);
        margin: var(--space-0) auto;
        padding: var(--space-8) var(--space-6);
    }

    .top {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: var(--space-4);
    }

    .card {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
        padding: var(--space-6);
        background-color: var(--color-surface);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-lg);
    }

    h2 {
        font-size: var(--font-size-md);
    }

    .grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
        gap: var(--space-6) var(--space-4);
    }

    .row {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-2);
    }

    .stack {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
    }

    .muted {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }
</style>
