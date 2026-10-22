<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'Badge, Spinner y Toast',
        group: 'Estado',
        description: 'Indicadores de estado y notificaciones.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import { toast } from '$lib/toast';

    import Badge from '../Badge.svelte';
    import Button from '../Button.svelte';
    import Spinner from '../Spinner.svelte';
</script>

<Story name="Badge">
    <Badge>neutral</Badge>
    <Badge variant="accent">accent</Badge>
    <Badge variant="success" dot>activo</Badge>
    <Badge variant="warning" dot>reiniciando</Badge>
    <Badge variant="danger" dot>fallido</Badge>
    <Badge variant="info">info</Badge>
</Story>

<Story name="Spinner">
    <Spinner size="sm" />
    <Spinner />
    <Spinner size="lg" label="Cargando" />
</Story>

<Story name="Toast">
    <Button onclick={() => toast.success('Despliegue completado', { description: 'api.service · main@3983fd9' })}>
        Éxito
    </Button>
    <Button onclick={() => toast.error('No se pudo conectar con el agente')}>Error</Button>
    <Button onclick={() => toast.warning('El servicio tardó en arrancar')}>Aviso</Button>
    <Button onclick={() => toast.info('Hay una nueva versión del agente', { duration: 0 })}>Info fija</Button>
</Story>
