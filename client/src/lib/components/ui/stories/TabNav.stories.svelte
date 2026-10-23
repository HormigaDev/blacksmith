<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'TabNav',
        group: 'Navegación',
        description: 'Pestañas que navegan entre rutas; la actual va marcada.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import TabNav from '../TabNav.svelte';
</script>

<Story name="Secciones" layout="stack">
    <div class="frame">
        <TabNav
            label="Secciones del servicio"
            items={[
                { href: '#logs', label: 'Logs', current: false },
                { href: '#deployment', label: 'Despliegue', current: true },
                { href: '#env', label: 'Variables', current: false },
            ]}
        />
    </div>
</Story>

<style>
    .frame {
        width: min(100%, var(--dialog-width-lg));
    }
</style>
