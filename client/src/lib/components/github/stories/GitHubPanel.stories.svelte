<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'GitHubPanel',
        group: 'GitHub',
        description: 'Conexión con GitHub: App del proyecto o propia, código de confirmación y cuenta conectada.',
    };
</script>

<script lang="ts">
    import { Story } from '$lib/workbench';

    import GitHubPanel from '../GitHubPanel.svelte';

    const settings = { clientId: 'Iv23liAbCdEf123456', appSlug: 'mi-blacksmith' };
    const handlers = {
        onConfigure: async () => {},
        onUseDefault: async () => {},
        onConnect: () => {},
        onCancel: () => {},
        onOpenVerification: () => {},
        onInstall: () => {},
        onDisconnect: async () => {},
    };
</script>

<Story name="Sin conectar" layout="stack">
    <div class="frame"><GitHubPanel {settings} user={null} pending={null} {...handlers} /></div>
</Story>

<Story name="App propia" layout="stack">
    <div class="frame"><GitHubPanel {settings} custom user={null} pending={null} {...handlers} /></div>
</Story>

<Story name="Confirmando código" layout="stack">
    <div class="frame">
        <GitHubPanel
            {settings}
            user={null}
            pending={{ userCode: 'WDJB-MJHT', verificationUri: 'https://github.com/login/device', expiresIn: 900 }}
            {...handlers}
        />
    </div>
</Story>

<Story name="Conectado" layout="stack">
    <div class="frame">
        <GitHubPanel {settings} user={{ login: 'octocat', name: null, avatarUrl: null }} pending={null} {...handlers} />
    </div>
</Story>

<style>
    .frame {
        width: min(100%, var(--dialog-width-md));
    }
</style>
