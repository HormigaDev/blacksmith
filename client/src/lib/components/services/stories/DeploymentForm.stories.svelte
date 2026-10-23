<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'DeploymentForm',
        group: 'Servicios',
        description: 'Repositorio, rama, preparación y arranque de un servicio.',
    };
</script>

<script lang="ts">
    import type { ServiceDeployment } from '$lib/agent';
    import { Story } from '$lib/workbench';

    import DeploymentForm from '../DeploymentForm.svelte';

    const empty: ServiceDeployment = {
        name: 'api',
        repository: null,
        access: 'account',
        workingDirectory: '/srv/bbel/api',
        setupCommands: [],
        startCommand: '',
        publishDir: null,
        updatedAt: null,
    };
    const defined: ServiceDeployment = {
        ...empty,
        repository: { owner: 'acme', name: 'api', branch: 'main' },
        setupCommands: ['npm ci', 'npm run build'],
        startCommand: 'node dist/server.js',
    };
    const repositories = [
        {
            owner: 'acme',
            name: 'api',
            fullName: 'acme/api',
            private: true,
            defaultBranch: 'main',
            htmlUrl: 'https://github.com/acme/api',
        },
    ];
    const loadBranches = async () => ['main', 'develop'];
    const onSave = async () => {};
    const noop = () => {};
</script>

<Story name="Sin GitHub" layout="stack">
    <div class="frame">
        <DeploymentForm
            deployment={empty}
            githubConnected={false}
            repositories={null}
            {loadBranches}
            onConnectGitHub={noop}
            {onSave}
        />
    </div>
</Story>

<Story name="Definido" layout="stack">
    <div class="frame">
        <DeploymentForm deployment={defined} githubConnected {repositories} {loadBranches} {onSave} />
    </div>
</Story>

<Story name="Eligiendo repositorio" layout="stack">
    <div class="frame">
        <DeploymentForm deployment={empty} githubConnected {repositories} {loadBranches} {onSave} />
    </div>
</Story>

<style>
    .frame {
        width: min(100%, var(--dialog-width-lg));
    }
</style>
