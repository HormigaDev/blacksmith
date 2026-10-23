<!-- Servidores en la barra lateral: cada uno enlaza a su página y muestra su estado con un punto. -->
<script lang="ts">
    import { t } from '$lib/i18n';
    import { serverAddress, serverHref } from '$lib/servers/address';
    import type { ServerEntry } from '$lib/servers/servers.svelte';

    interface Props {
        entries: readonly ServerEntry[];
        /** Ruta actual, para marcar el servidor abierto. */
        currentPath: string;
    }

    let { entries, currentPath }: Props = $props();

    const isCurrent = (id: string) => {
        const href = serverHref(id);
        return currentPath === href || currentPath.startsWith(`${href}/`);
    };
</script>

<nav class="nav" aria-label={t('servers.title')}>
    <ul class="list">
        {#each entries as entry (entry.profile.id)}
            <li>
                <a
                    class="item"
                    href={serverHref(entry.profile.id)}
                    aria-current={isCurrent(entry.profile.id) ? 'page' : undefined}
                >
                    <span class="dot {entry.status}" aria-hidden="true"></span>
                    <span class="label">{serverAddress(entry.profile)}</span>
                    <span class="visually-hidden">{t(`servers.status.${entry.status}`)}</span>
                </a>
            </li>
        {/each}
    </ul>
</nav>

<style>
    .list {
        display: flex;
        flex-direction: column;
        gap: var(--space-px);
        padding: var(--space-0);
        margin: var(--space-0);
        list-style: none;
    }

    .item {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-2) var(--space-3);
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
        text-decoration: none;
        border-radius: var(--radius-md);
        transition:
            background-color var(--duration-fast) var(--easing-standard),
            color var(--duration-fast) var(--easing-standard);
    }

    .item:hover {
        color: var(--color-text);
        background-color: var(--color-surface-hover);
    }

    .item[aria-current='page'] {
        font-weight: var(--font-weight-medium);
        color: var(--color-text);
        background-color: var(--color-surface);
        box-shadow: var(--shadow-sm);
    }

    .item:focus-visible {
        outline: var(--focus-ring-width) solid var(--color-focus-ring);
        outline-offset: calc(-1 * var(--focus-ring-width));
    }

    .label {
        min-width: 0;
        overflow: hidden;
        font-family: var(--font-mono);
        font-size: var(--font-size-xs);
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .dot {
        flex-shrink: 0;
        width: var(--space-2);
        height: var(--space-2);
        background-color: var(--color-text-disabled);
        border-radius: var(--radius-full);
    }

    .dot.connected {
        background-color: var(--color-success);
    }

    .dot.connecting {
        background-color: var(--color-info);
    }

    .dot.agentMissing {
        background-color: var(--color-warning);
    }

    .dot.error {
        background-color: var(--color-danger);
    }

    .visually-hidden {
        position: absolute;
        width: 1px;
        height: 1px;
        margin: calc(-1 * var(--space-px));
        padding: var(--space-0);
        overflow: hidden;
        clip: rect(0 0 0 0);
        white-space: nowrap;
        border: 0;
    }
</style>
