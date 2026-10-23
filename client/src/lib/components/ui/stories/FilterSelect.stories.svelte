<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'FilterSelect',
        group: 'Selección',
        description:
            'Lista desplegable con búsqueda para listas largas: filtra al escribir y la búsqueda se vacía al elegir.',
    };
</script>

<script lang="ts">
    import GitBranchIcon from '@lucide/svelte/icons/git-branch';

    import { Story } from '$lib/workbench';

    import FilterSelect from '../FilterSelect.svelte';

    const repositories = [
        'acme/api-pagos',
        'acme/web',
        'acme/bot-discord',
        'acme/infra',
        'hormiga/blacksmith',
        'hormiga/notas',
    ].map((value) => ({ value, label: value }));
    const branches = ['main', 'develop', 'release/1.4', 'hotfix/login'].map((value) => ({ value, label: value }));

    let repository = $state<string>();
    let branch = $state('main');
</script>

<Story name="Básico" layout="grid">
    <FilterSelect
        label="Repositorio"
        placeholder="Elige un repositorio"
        searchPlaceholder="Buscar repositorio…"
        options={repositories}
        bind:value={repository}
    />
    <FilterSelect label="Rama" options={branches} bind:value={branch}>
        {#snippet leading()}<GitBranchIcon />{/snippet}
    </FilterSelect>
</Story>

<Story name="Estados" layout="grid">
    <FilterSelect label="Con error" options={branches} error="Elige una rama" />
    <FilterSelect label="Deshabilitado" options={branches} value="main" disabled />
</Story>
