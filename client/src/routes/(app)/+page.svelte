<script lang="ts">
    import { APP_NAME } from '$lib/app';
    import { t } from '$lib/i18n';
</script>

<svelte:head>
    <title>{t('dashboard.title')} · {APP_NAME}</title>
</svelte:head>

<section class="empty">
    <h1 class="title">{t('dashboard.empty.title')}</h1>
    <p class="description">{t('dashboard.empty.description')}</p>
</section>

<style>
    .empty {
        display: grid;
        place-content: center;
        gap: var(--space-2);
        min-height: 100%;
        text-align: center;
    }

    .title {
        font-size: var(--font-size-xl);
    }

    .description {
        color: var(--color-text-muted);
        font-size: var(--font-size-sm);
    }
</style>
