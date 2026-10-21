<!-- Marco de la ventana principal: cabecera fija y área de contenido con scroll propio. -->
<script lang="ts">
    import type { Snippet } from 'svelte';

    import { APP_SHORT_NAME } from '$lib/app';
    import LocaleSwitcher from '$lib/components/locale/LocaleSwitcher.svelte';
    import ThemeSwitcher from '$lib/components/theme/ThemeSwitcher.svelte';

    let { children }: { children: Snippet } = $props();
</script>

<div class="shell">
    <header class="header">
        <span class="brand">{APP_SHORT_NAME}</span>
        <div class="actions">
            <LocaleSwitcher />
            <ThemeSwitcher />
        </div>
    </header>

    <main class="content">
        {@render children()}
    </main>
</div>

<style>
    .shell {
        display: grid;
        grid-template-rows: var(--layout-header-height) minmax(0, 1fr);
        height: 100vh;
    }

    .header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
        padding: var(--space-0) var(--space-4);
        background-color: var(--color-surface);
        border-bottom: var(--border-width) solid var(--color-border);
    }

    .brand {
        font-weight: var(--font-weight-semibold);
        color: var(--color-text);
    }

    .actions {
        display: flex;
        align-items: center;
        gap: var(--space-4);
    }

    .content {
        overflow: auto;
        padding: var(--space-6);
    }
</style>
