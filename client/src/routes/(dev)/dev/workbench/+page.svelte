<script lang="ts">
    import { STORIES } from '$lib/workbench';
</script>

<section class="intro">
    <h1>Workbench</h1>
    <p>
        {STORIES.length} componentes con sus variantes, aislados de la app. Los cambios en los componentes se reflejan al
        guardar.
    </p>
    <p>
        Cada componente define sus variantes en <code>stories/NombreDelComponente.stories.svelte</code>, junto a la
        carpeta del componente. Elige uno en el menú o usa <code>?story=</code> para ver una sola variante.
    </p>
</section>

<style>
    .intro {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
        max-width: var(--dialog-width-lg);
        color: var(--color-text-muted);
    }

    h1 {
        font-size: var(--font-size-2xl);
        color: var(--color-text);
    }
</style>
