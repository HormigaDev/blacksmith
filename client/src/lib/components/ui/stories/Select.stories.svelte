<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'Select',
        group: 'Selección',
        description: 'Lista desplegable accesible: teclado, typeahead y opciones deshabilitadas.',
    };
</script>

<script lang="ts">
    import GitBranchIcon from '@lucide/svelte/icons/git-branch';

    import { Story } from '$lib/workbench';

    import Select from '../Select.svelte';

    const units = [
        { value: 'nginx.service', label: 'nginx.service' },
        { value: 'api.service', label: 'api.service' },
        { value: 'worker.service', label: 'worker.service' },
        { value: 'legacy.service', label: 'legacy.service (deshabilitado)', disabled: true },
    ];
    const branches = ['main', 'develop', 'release/1.4', 'hotfix/login'].map((value) => ({ value, label: value }));
    const many = Array.from({ length: 40 }, (_, index) => ({
        value: `srv-${index + 1}`,
        label: `servidor-${index + 1}`,
    }));

    let unit = $state<string>();
    let branch = $state('main');
</script>

<Story name="Básico" layout="grid">
    <Select label="Unidad systemd" options={units} bind:value={unit} />
    <Select label="Rama" options={branches} bind:value={branch}>
        {#snippet leading()}<GitBranchIcon />{/snippet}
    </Select>
    <Select label="Lista larga" options={many} placeholder="Elige un servidor" hint="Hace scroll dentro del menú" />
</Story>

<Story name="Estados" layout="grid">
    <Select label="Obligatorio" options={units} required />
    <Select label="Con error" options={units} error="Selecciona una unidad" />
    <Select label="Deshabilitado" options={units} value="api.service" disabled />
</Story>

<Story name="Tamaños" layout="grid">
    <Select label="Pequeño" size="sm" options={branches} value="main" />
    <Select label="Mediano" options={branches} value="main" />
    <Select label="Grande" size="lg" options={branches} value="main" />
</Story>
