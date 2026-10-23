<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'DeployKeyPanel',
        group: 'Servicios',
        description: 'Llave de despliegue opcional: generarla y añadirla al repositorio.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import DeployKeyPanel from '../DeployKeyPanel.svelte';

    const key = {
        publicKey:
            'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl blacksmith-api@vps',
        fingerprint: 'SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU',
    };
    const repository = { owner: 'acme', name: 'api', branch: 'main' };
    const handlers = { onCreate: async () => {}, onRegister: async () => {}, onOpenGitHub: () => {} };
</script>

<Story name="Sin llave" layout="stack">
    <div class="frame"><DeployKeyPanel key={null} {repository} canRegister={false} {...handlers} /></div>
</Story>

<Story name="Registro manual" layout="stack">
    <div class="frame"><DeployKeyPanel {key} {repository} canRegister={false} {...handlers} /></div>
</Story>

<Story name="Con App propia" layout="stack">
    <div class="frame"><DeployKeyPanel {key} {repository} canRegister {...handlers} /></div>
</Story>

<style>
    .frame {
        width: min(100%, var(--dialog-width-lg));
    }
</style>
