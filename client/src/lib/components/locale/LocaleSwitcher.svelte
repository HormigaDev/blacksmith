<script lang="ts">
    import Select from '$lib/components/ui/Select.svelte';
    import { availableLocales, i18n, type LocalePreference, t } from '$lib/i18n';

    // Los nombres de idioma son autónimos ("English", "Português"): no se traducen.
    const options = $derived<readonly { value: LocalePreference; label: string }[]>([
        { value: 'system', label: t('common.system') },
        ...availableLocales().map(({ code, name }) => ({ value: code as LocalePreference, label: name })),
    ]);
</script>

<Select
    label={t('locale.label')}
    {options}
    bind:value={() => i18n.preference, (preference) => i18n.setPreference(preference)}
/>
