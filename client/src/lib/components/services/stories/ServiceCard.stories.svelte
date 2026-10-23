<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'ServiceCard',
        group: 'Servicios',
        description: 'Un servicio registrado en el agente.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import ServiceCard from '../ServiceCard.svelte';

    const noop = () => {};
    const base = { name: 'api', displayName: 'API de pagos', unit: 'bbel-api.service', createdAt: 0 };
</script>

<Story name="Con descripción" layout="stack">
    <div class="frame">
        <ServiceCard
            service={{ ...base, description: 'Cobros con tarjeta y transferencias.\nMantiene: equipo de pagos.' }}
            href="#"
            onRemove={noop}
        />
    </div>
</Story>

<Story name="Sin descripción" layout="stack">
    <div class="frame">
        <ServiceCard service={{ ...base, description: '' }} href="#" onRemove={noop} />
    </div>
</Story>

<style>
    .frame {
        width: var(--layout-card-min-width);
    }
</style>
