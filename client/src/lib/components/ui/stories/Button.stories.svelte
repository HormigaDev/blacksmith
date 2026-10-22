<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'Button',
        group: 'Acciones',
        description: 'Acción principal, secundaria, discreta o destructiva. type="button" por defecto.',
    };
</script>

<script lang="ts">
    import PlayIcon from '@lucide/svelte/icons/play';
    import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
    import Trash2Icon from '@lucide/svelte/icons/trash-2';

    import { Story } from '$lib/workbench';

    import Button from '../Button.svelte';
</script>

<Story name="Variantes">
    <Button variant="primary">Desplegar</Button>
    <Button>Secundario</Button>
    <Button variant="ghost">Ghost</Button>
    <Button variant="danger">Detener</Button>
</Story>

<Story name="Tamaños">
    <Button size="sm">Pequeño</Button>
    <Button>Mediano</Button>
    <Button size="lg" variant="primary">Grande</Button>
</Story>

<Story name="Con ícono">
    <Button variant="primary"><PlayIcon /> Iniciar</Button>
    <Button><RefreshCwIcon /> Reiniciar</Button>
    <Button variant="danger"><Trash2Icon /> Eliminar</Button>
</Story>

<Story name="Solo ícono" description="Requiere aria-label.">
    <Button iconOnly aria-label="Reiniciar"><RefreshCwIcon /></Button>
    <Button iconOnly variant="ghost" aria-label="Eliminar"><Trash2Icon /></Button>
    <Button iconOnly variant="primary" size="sm" aria-label="Iniciar"><PlayIcon /></Button>
</Story>

<Story name="Estados">
    <Button variant="primary" loading>Guardando</Button>
    <Button loading iconOnly aria-label="Reiniciando"><RefreshCwIcon /></Button>
    <Button disabled>Deshabilitado</Button>
    <Button variant="primary" disabled>Deshabilitado</Button>
</Story>

<Story name="Ancho completo" layout="stack">
    <Button variant="primary" fullWidth>Conectar al servidor</Button>
</Story>
