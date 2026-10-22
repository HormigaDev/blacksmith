<!--
    Ícono de Font Awesome Free (solid, regular, brands) sin `fontawesome-svg-core`: dibuja el SVG
    a partir de la definición del ícono, sin CSS inyectado en tiempo de ejecución.

        import { faGithub } from '@fortawesome/free-brands-svg-icons';
        <FaIcon icon={faGithub} />

    Importar siempre por nombre: los paquetes son `sideEffects: false` y solo se empaqueta lo usado.
    Lucide sigue siendo el set por defecto de la interfaz; ver client/docs/components.md §3.
-->
<script lang="ts">
    import type { IconDefinition } from '@fortawesome/fontawesome-common-types';

    interface Props {
        icon: IconDefinition;
        size?: 'sm' | 'md' | 'lg';
        /** Ancho fijo (cuadrado): alinea columnas de íconos de distinta proporción. */
        fixedWidth?: boolean;
        /** Si se indica, el ícono es informativo y se anuncia; si no, es decorativo. */
        label?: string;
        spin?: boolean;
    }

    let { icon, size = 'md', fixedWidth = false, label, spin = false }: Props = $props();

    const width = $derived(icon.icon[0]);
    const height = $derived(icon.icon[1]);
    // En Font Awesome Free es un único path; los duotono (Pro) traen dos.
    const paths = $derived([icon.icon[4]].flat());
</script>

<svg
    class="fa {size}"
    class:fixed-width={fixedWidth}
    class:spin
    viewBox="0 0 {width} {height}"
    fill="currentColor"
    focusable="false"
    role={label ? 'img' : undefined}
    aria-label={label}
    aria-hidden={label ? undefined : 'true'}
    data-icon="{icon.prefix}-{icon.iconName}"
>
    {#each paths as d, index (index)}
        <path {d} />
    {/each}
</svg>

<style>
    .fa {
        display: inline-block;
        flex-shrink: 0;
        width: auto;
        height: var(--icon-size-md);
        overflow: visible;
        vertical-align: -0.125em;
    }

    .sm {
        height: var(--icon-size-sm);
    }

    .lg {
        height: var(--icon-size-lg);
    }

    .fixed-width {
        aspect-ratio: 1;
    }

    .spin {
        animation: fa-spin var(--duration-spin) var(--easing-linear) infinite;
    }

    @keyframes fa-spin {
        to {
            transform: rotate(1turn);
        }
    }
</style>
