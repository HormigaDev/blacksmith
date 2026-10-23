<!--
    Botón de GitHub: muestra la cuenta conectada y abre el panel de conexión. El estado lo carga el
    layout de la aplicación (`github.load()`). Solo existe en la aplicación de escritorio: fuera de Tauri no hay GitHub (ni `preview`, para el
    Workbench).
-->
<script lang="ts">
    import { faGithub } from '@fortawesome/free-brands-svg-icons';
    import { isTauri } from '@tauri-apps/api/core';
    import { openUrl } from '@tauri-apps/plugin-opener';

    import { github } from '$lib/github';
    import { t } from '$lib/i18n';

    import Button from '../ui/Button.svelte';
    import Dialog from '../ui/Dialog.svelte';
    import FaIcon from '../ui/FaIcon.svelte';
    import GitHubPanel from './GitHubPanel.svelte';

    interface Props {
        /** Mostrar aunque no haya Tauri (solo para el Workbench). */
        preview?: boolean;
    }

    let { preview = false }: Props = $props();

    const native = isTauri();
</script>

{#if native || preview}
    <Button size="sm" variant="ghost" onclick={() => (github.panelOpen = true)}>
        <FaIcon icon={faGithub} />
        {github.user ? t('github.connectedAs', { login: github.user.login }) : t('github.button')}
    </Button>

    <Dialog bind:open={github.panelOpen} title={t('github.title')} size="sm">
        <GitHubPanel
            settings={github.settings}
            custom={github.custom}
            user={github.user}
            pending={github.pending}
            busy={github.busy}
            error={github.error}
            onConfigure={(settings) => github.configure(settings)}
            onUseDefault={() => github.useDefault()}
            onConnect={() => github.connect()}
            onCancel={() => github.cancel()}
            onOpenVerification={() => {
                if (github.pending) void openUrl(github.pending.verificationUri);
            }}
            onInstall={() => github.openInstall()}
            onDisconnect={() => github.disconnect()}
        />
    </Dialog>
{/if}
