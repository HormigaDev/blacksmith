<script lang="ts">
    import Select from '$lib/components/ui/Select.svelte';
    import { t } from '$lib/i18n';
    import { theme, THEMES, type ThemePreference } from '$lib/theme';

    const options = $derived<readonly { value: ThemePreference; label: string }[]>([
        { value: 'system', label: t('common.system') },
        ...THEMES.map(({ id }) => ({ value: id, label: t(`theme.names.${id}`) })),
    ]);
</script>

<Select label={t('theme.label')} {options} bind:value={theme.preference} />
