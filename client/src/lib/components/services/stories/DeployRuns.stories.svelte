<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'DeployRuns',
        group: 'Servicios',
        description: 'Lanzar un despliegue y los últimos, con su resultado.',
    };
</script>

<script lang="ts">
    import type { DeployRun } from '$lib/agent';
    import { Story } from '$lib/workbench';

    import DeployRuns from '../DeployRuns.svelte';

    const now = 1_800_000_000_000;
    const at = (minutes: number) => now / 1000 - minutes * 60;
    const history: DeployRun[] = [
        {
            id: 3,
            status: 'failed',
            kind: 'deploy',
            release: 'r1799999700-000000000',
            branch: 'main',
            startedAt: at(5),
            finishedAt: at(4),
            detail: 'npm ci falló',
        },
        {
            id: 2,
            status: 'succeeded',
            kind: 'deploy',
            release: 'r1799994600-000000000',
            branch: 'main',
            startedAt: at(90),
            finishedAt: at(88),
            detail: null,
        },
        {
            id: 1,
            status: 'succeeded',
            kind: 'deploy',
            release: 'r1799820000-000000000',
            branch: 'develop',
            startedAt: at(3000),
            finishedAt: at(2998),
            detail: null,
        },
    ];
    const running: DeployRun = {
        id: 4,
        status: 'running',
        kind: 'deploy',
        release: 'r1799999940-000000000',
        branch: 'main',
        startedAt: at(1),
        finishedAt: null,
        detail: null,
    };
    const onDeploy = () => {};
</script>

{#snippet output()}
    <pre class="output">==&gt; Clonando https://github.com/acme/api.git (main)
==&gt; [1/2] npm ci
npm error code ERESOLVE</pre>
{/snippet}

<Story name="Historial" layout="stack">
    <div class="frame">
        <DeployRuns
            runs={history}
            blocked={null}
            activeRelease={history[1]?.release}
            previousRelease={history[2]?.release}
            {onDeploy}
            onRollback={async () => {}}
            {output}
            {now}
        />
    </div>
</Story>

<Story name="En curso" layout="stack">
    <div class="frame"><DeployRuns runs={[running, ...history]} blocked={null} {onDeploy} {output} {now} /></div>
</Story>

<Story name="Bloqueado" layout="stack">
    <div class="frame"><DeployRuns runs={[]} blocked="Define el comando de arranque." {onDeploy} {now} /></div>
</Story>

<style>
    .output {
        padding: var(--space-3);
        margin: var(--space-0);
        font-family: var(--font-mono);
        font-size: var(--font-size-xs);
        background-color: var(--color-surface);
        border-radius: var(--radius-sm);
    }

    .frame {
        width: min(100%, var(--dialog-width-lg));
    }
</style>
