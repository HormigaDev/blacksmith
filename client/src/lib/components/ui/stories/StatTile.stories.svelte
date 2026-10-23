<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'StatTile',
        group: 'Datos',
        description: 'Indicador de un vistazo: etiqueta, valor, detalle y su tendencia o medidor.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import Meter from '../Meter.svelte';
    import Sparkline from '../Sparkline.svelte';
    import StatTile from '../StatTile.svelte';

    const values = [120, 130, 128, 140, 156, 150, 149, 160, 171, 168];
</script>

<Story name="Variantes" layout="grid">
    <div class="frame"><StatTile label="Procesos e hilos" value="7" detail="Todos los del servicio" /></div>
    <div class="frame">
        <StatTile label="Web" value="https://api.example.com" detail="HTTPS activo" kind="text" />
    </div>
    <div class="frame">
        <StatTile label="Memoria" value="168 MB" detail="Pico 190 MB">
            <Meter label="Memoria frente a su límite" value={168} max={512} />
        </StatTile>
    </div>
    <div class="frame">
        <StatTile label="Memoria" value="168 MB">
            <Sparkline label="Memoria" {values} format={(value) => `${value} MB`} />
        </StatTile>
    </div>
</Story>

<style>
    .frame {
        width: var(--layout-tile-min-width);
    }
</style>
