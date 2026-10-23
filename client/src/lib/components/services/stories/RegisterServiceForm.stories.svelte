<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'RegisterServiceForm',
        group: 'Servicios',
        description: 'Alta de un servicio en el agente (solo datos informativos).',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import RegisterServiceForm from '../RegisterServiceForm.svelte';

    const onSubmit = async () => {};
</script>

<Story name="Vacío" layout="stack">
    <div class="frame">
        <RegisterServiceForm {onSubmit} />
    </div>
</Story>

<style>
    .frame {
        width: min(100%, var(--dialog-width-lg));
    }
</style>
