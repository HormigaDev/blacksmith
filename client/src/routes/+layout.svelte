<script lang="ts">
    import '$lib/styles/global.css';

    import type { Snippet } from 'svelte';

    import Toaster from '$lib/components/ui/Toaster.svelte';
    import { i18n } from '$lib/i18n';
    import { applyTheme, theme } from '$lib/theme';

    let { children }: { children: Snippet } = $props();

    $effect(() => theme.followSystem());
    $effect(() => applyTheme(theme.current));
    $effect(() => {
        document.documentElement.lang = i18n.current.intl;
    });
</script>

{@render children()}

<Toaster />
