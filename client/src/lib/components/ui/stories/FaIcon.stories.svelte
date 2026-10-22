<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'FaIcon',
        group: 'Íconos',
        description: 'Font Awesome Free (solid, regular, brands). Lucide es el set por defecto de la interfaz.',
    };
</script>

<script lang="ts">
    import { faBell, faCircleCheck, faClock } from '@fortawesome/free-regular-svg-icons';
    import { faDebian, faGithub, faLinux, faNodeJs, faRust, faUbuntu } from '@fortawesome/free-brands-svg-icons';
    import { faDatabase, faRotate, faServer, faTerminal } from '@fortawesome/free-solid-svg-icons';

    import { Story } from '$lib/workbench';

    import Badge from '../Badge.svelte';
    import Button from '../Button.svelte';
    import FaIcon from '../FaIcon.svelte';
</script>

<Story name="Marcas" description="El uso principal: GitHub, distribuciones y runtimes.">
    {#each [faGithub, faLinux, faUbuntu, faDebian, faNodeJs, faRust] as icon (icon.iconName)}
        <FaIcon {icon} size="lg" label={icon.iconName} />
    {/each}
</Story>

<Story name="Solid y regular">
    {#each [faServer, faDatabase, faTerminal, faRotate, faBell, faClock, faCircleCheck] as icon (icon.iconName)}
        <FaIcon {icon} />
    {/each}
</Story>

<Story name="Tamaños y ancho fijo">
    <FaIcon icon={faServer} size="sm" />
    <FaIcon icon={faServer} />
    <FaIcon icon={faServer} size="lg" />
    <FaIcon icon={faTerminal} fixedWidth />
    <FaIcon icon={faRotate} spin label="Sincronizando" />
</Story>

<Story name="Combinado">
    <Button><FaIcon icon={faGithub} size="sm" /> Conectar con GitHub</Button>
    <Badge variant="success"><FaIcon icon={faCircleCheck} size="sm" /> Sincronizado</Badge>
</Story>
