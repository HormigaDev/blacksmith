<!--
    Alta de un servicio en el agente: solo datos informativos (no toca systemd). El identificador
    define la unidad `blacksmith-<identificador>.service`; se muestra mientras se escribe.
-->
<script lang="ts">
    import { type RegisterService, SERVICE_NAME_PATTERN } from '$lib/agent';
    import { describeError, HttpError } from '$lib/http';
    import { t } from '$lib/i18n';

    import Button from '../ui/Button.svelte';
    import Input from '../ui/Input.svelte';
    import Textarea from '../ui/Textarea.svelte';

    type Field = 'name' | 'displayName' | 'description';

    interface Props {
        /** Registra el servicio. Si lanza, el error se muestra en el formulario. */
        onSubmit: (service: RegisterService) => Promise<void>;
    }

    let { onSubmit }: Props = $props();

    let name = $state('');
    let displayName = $state('');
    let description = $state('');
    let busy = $state(false);
    let fieldErrors = $state<Partial<Record<Field, string>>>({});
    let formError = $state<string>();

    const unit = $derived(`blacksmith-${name.trim() || '…'}.service`);

    function validate(): boolean {
        const errors: Partial<Record<Field, string>> = {};
        if (!name.trim()) errors.name = t('services.register.required');
        else if (!SERVICE_NAME_PATTERN.test(name.trim())) errors.name = t('services.register.nameInvalid');
        if (!displayName.trim()) errors.displayName = t('services.register.required');
        fieldErrors = errors;
        return Object.keys(errors).length === 0;
    }

    async function submit(event: SubmitEvent): Promise<void> {
        event.preventDefault();
        formError = undefined;
        if (!validate()) return;
        busy = true;
        try {
            await onSubmit({ name: name.trim(), displayName: displayName.trim(), description: description.trim() });
        } catch (error) {
            if (error instanceof HttpError && error.code === 'SERVICE_ALREADY_EXISTS') {
                fieldErrors = { name: describeError(error) };
            } else {
                formError = describeError(error);
            }
        } finally {
            busy = false;
        }
    }
</script>

<form class="form" novalidate onsubmit={submit}>
    <Input
        label={t('services.register.name')}
        hint={t('services.register.nameHint', { unit })}
        placeholder={t('services.register.namePlaceholder')}
        required
        autocomplete="off"
        spellcheck="false"
        maxlength={48}
        error={fieldErrors.name}
        bind:value={name}
    />
    <Input
        label={t('services.register.displayName')}
        placeholder={t('services.register.displayNamePlaceholder')}
        required
        autocomplete="off"
        maxlength={100}
        error={fieldErrors.displayName}
        bind:value={displayName}
    />
    <Textarea
        label={t('services.register.descriptionLabel')}
        placeholder={t('services.register.descriptionPlaceholder')}
        rows={3}
        maxlength={500}
        error={fieldErrors.description}
        bind:value={description}
    />

    {#if formError}
        <p class="alert" role="alert">{formError}</p>
    {/if}

    <div class="actions">
        <Button type="submit" variant="primary" loading={busy}>{t('services.register.submit')}</Button>
    </div>
</form>

<style>
    .form {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
    }

    .alert {
        padding: var(--space-3);
        font-size: var(--font-size-sm);
        color: var(--color-danger);
        background-color: var(--color-danger-subtle);
        border-radius: var(--radius-md);
    }

    .actions {
        display: flex;
        justify-content: flex-end;
    }
</style>
