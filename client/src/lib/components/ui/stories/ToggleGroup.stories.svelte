<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'ToggleGroup',
        group: 'Selección',
        description: 'Selección única segmentada.',
    };
</script>

<script lang="ts">
    import LayoutGridIcon from '@lucide/svelte/icons/layout-grid';
    import ListIcon from '@lucide/svelte/icons/list';
    import TableIcon from '@lucide/svelte/icons/table';

    import { Story } from '$lib/workbench';

    import ToggleGroup from '../ToggleGroup.svelte';

    const ranges = [
        { value: '1h', label: '1 h' },
        { value: '24h', label: '24 h' },
        { value: '7d', label: '7 días' },
        { value: '30d', label: '30 días', disabled: true },
    ];
    const views = [
        { value: 'list', label: 'Lista', icon: ListIcon },
        { value: 'grid', label: 'Cuadrícula', icon: LayoutGridIcon },
        { value: 'table', label: 'Tabla', icon: TableIcon },
    ];
</script>

<Story name="Texto">
    <ToggleGroup label="Rango" options={ranges} value="24h" />
    <ToggleGroup label="Rango pequeño" size="sm" options={ranges} value="1h" />
</Story>

<Story name="Íconos">
    <ToggleGroup label="Vista" options={views} iconOnly value="list" />
    <ToggleGroup label="Vista con texto" options={views} value="grid" />
</Story>

<Story name="Deshabilitado">
    <ToggleGroup label="Rango" options={ranges} value="24h" disabled />
</Story>
