<!--
    Pestañas de navegación: cada pestaña es un enlace a su propia ruta (se puede recargar, compartir
    o volver atrás). La actual lleva `aria-current="page"`. Para pestañas sin navegación dentro de
    una misma vista, úsese `ToggleGroup`.
-->
<script lang="ts">
    interface Item {
        href: string;
        label: string;
        current: boolean;
    }

    interface Props {
        /** Nombre accesible de la navegación. */
        label: string;
        items: readonly Item[];
    }

    let { label, items }: Props = $props();
</script>

<nav aria-label={label}>
    <ul class="list">
        {#each items as item (item.href)}
            <li>
                <a class="tab" href={item.href} aria-current={item.current ? 'page' : undefined}>{item.label}</a>
            </li>
        {/each}
    </ul>
</nav>

<style>
    /* La línea base va dentro de la lista (sombra interior) y el subrayado de la pestaña activa
       la tapa sin salirse: nada desborda en vertical, así que no aparece una barra de scroll. */
    .list {
        display: flex;
        gap: var(--space-1);
        padding: var(--space-0);
        margin: var(--space-0);
        overflow-x: auto;
        overflow-y: hidden;
        list-style: none;
        box-shadow: var(--shadow-divider-bottom);
    }

    .tab {
        display: inline-flex;
        align-items: center;
        padding: var(--space-2) var(--space-3);
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-medium);
        color: var(--color-text-muted);
        text-decoration: none;
        white-space: nowrap;
        border-bottom: var(--border-width-strong) solid transparent;
        transition:
            color var(--duration-fast) var(--easing-standard),
            border-color var(--duration-fast) var(--easing-standard);
    }

    .tab:hover {
        color: var(--color-text);
    }

    .tab[aria-current='page'] {
        color: var(--color-text);
        border-bottom-color: var(--color-accent);
    }

    .tab:focus-visible {
        outline: var(--focus-ring-width) solid var(--color-focus-ring);
        outline-offset: calc(-1 * var(--focus-ring-width));
        border-radius: var(--radius-sm);
    }
</style>
