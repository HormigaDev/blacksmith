<script lang="ts">
    import LanguagesIcon from '@lucide/svelte/icons/languages';

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
    hideLabel
    size="sm"
    {options}
    bind:value={() => i18n.preference, (preference) => preference && i18n.setPreference(preference)}
>
    {#snippet leading()}<LanguagesIcon />{/snippet}
</Select>
