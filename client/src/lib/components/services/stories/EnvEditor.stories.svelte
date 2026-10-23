<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'EnvEditor',
        group: 'Servicios',
        description: 'Variables de entorno: globales del servidor o de un servicio (que sobrescribe).',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import EnvEditor from '../EnvEditor.svelte';

    const onSave = async () => {};
</script>

<Story name="Globales" layout="stack">
    <div class="frame">
        <EnvEditor
            variables={[
                { key: 'NODE_ENV', value: 'production' },
                { key: 'LOG_LEVEL', value: 'info' },
            ]}
            {onSave}
        />
    </div>
</Story>

<Story name="Servicio con heredadas" layout="stack">
    <div class="frame">
        <EnvEditor
            variables={[
                { key: 'DATABASE_URL', value: 'postgres://api@localhost/api' },
                { key: 'LOG_LEVEL', value: 'debug' },
            ]}
            inherited={[
                { key: 'NODE_ENV', value: 'production' },
                { key: 'LOG_LEVEL', value: 'info' },
            ]}
            {onSave}
        />
    </div>
</Story>

<Story name="Vacío" layout="stack">
    <div class="frame"><EnvEditor variables={[]} inherited={[]} {onSave} /></div>
</Story>

<style>
    .frame {
        width: min(100%, var(--dialog-width-lg));
    }
</style>
