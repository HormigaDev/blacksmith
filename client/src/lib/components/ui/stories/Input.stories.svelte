<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'Input',
        group: 'Formularios',
        description: 'Único input de texto de la app. Íconos con los snippets leading y trailing.',
    };
</script>

<script lang="ts">
    import AtSignIcon from '@lucide/svelte/icons/at-sign';
    import GlobeIcon from '@lucide/svelte/icons/globe';
    import SearchIcon from '@lucide/svelte/icons/search';
    import ServerIcon from '@lucide/svelte/icons/server';

    import { Story } from '$lib/workbench';

    import Input from '../Input.svelte';

    let port = $state<number | null>(22);
    let replicas = $state<number | null>(1);
</script>

<Story name="Tipos" layout="grid">
    <Input label="Texto" placeholder="mi-servidor" />
    <Input label="Email" type="email" placeholder="ops@example.com" />
    <Input label="Contraseña" type="password" value="un-secreto" />
    <Input label="Buscar" type="search" placeholder="Filtrar servicios…" />
    <Input label="URL" type="url" placeholder="https://hooks.example.com" />
    <Input label="Teléfono" type="tel" placeholder="+55 11 0000-0000" />
</Story>

<Story name="Número" layout="grid" description="Enfoca el campo: los controles aparecen por fuera del borde.">
    <Input label="Puerto SSH" type="number" min={1} max={65535} bind:value={port} />
    <Input label="Réplicas" type="number" min={1} max={8} hint="Entre 1 y 8" bind:value={replicas} />
    <Input label="Paso de 0,5" type="number" step={0.5} value={1.5} />
    <Input label="Sin límites" type="number" value={0} />
</Story>

<Story name="Íconos" layout="grid">
    <Input label="Buscar" type="search" placeholder="Buscar…">
        {#snippet leading()}<SearchIcon />{/snippet}
    </Input>
    <Input label="Host" placeholder="vps.example.com">
        {#snippet trailing()}<ServerIcon />{/snippet}
    </Input>
    <Input label="Dominio" placeholder="example.com">
        {#snippet leading()}<GlobeIcon />{/snippet}
        {#snippet trailing()}<AtSignIcon />{/snippet}
    </Input>
</Story>

<Story name="Estados" layout="grid">
    <Input label="Con ayuda" hint="Nombre o IP del servidor" />
    <Input label="Obligatorio" required />
    <Input label="Con error" value="no-es-un-host" error="El host no es válido" />
    <Input label="Deshabilitado" value="solo lectura" disabled />
    <Input label="Solo lectura" value="127.0.0.1:4040" readonly />
    <Input label="Etiqueta oculta" hideLabel placeholder="Etiqueta solo para lectores de pantalla" />
</Story>

<Story name="Tamaños" layout="grid">
    <Input label="Pequeño" size="sm" placeholder="sm" />
    <Input label="Mediano" placeholder="md" />
    <Input label="Grande" size="lg" placeholder="lg" />
</Story>
