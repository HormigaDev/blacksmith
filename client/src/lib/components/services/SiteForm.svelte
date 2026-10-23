<!--
    Sitio web del servicio detrás de nginx: tipo de proyecto (plantilla), dominios, puerto del
    servicio y HTTPS con Let's Encrypt. Valida como el agente antes de guardar; el agente genera la
    configuración, la prueba con `nginx -t` y, si nginx la rechaza, mantiene la anterior.

    Estado del certificado: se pide en segundo plano tras guardar (el sitio ya responde por HTTP);
    si falla, se muestra el motivo y se puede reintentar.
-->
<script lang="ts">
    import ExternalLinkIcon from '@lucide/svelte/icons/external-link';
    import Trash2Icon from '@lucide/svelte/icons/trash-2';
    import { untrack } from 'svelte';

    import { proxies, servesFiles, type Site, type SiteConfig, SITE_TEMPLATES, type SiteTemplate } from '$lib/agent';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';

    import Badge from '../ui/Badge.svelte';
    import Button from '../ui/Button.svelte';
    import ConfirmDialog from '../ui/ConfirmDialog.svelte';
    import Input from '../ui/Input.svelte';
    import Select from '../ui/Select.svelte';
    import Switch from '../ui/Switch.svelte';

    interface Props {
        /** `null` si el servicio aún no tiene sitio. */
        site: Site | null;
        /** Carpeta pública del despliegue (`null` si no publica archivos). */
        publishDir: string | null;
        /** Puerto del agente: ningún sitio puede usarlo. */
        agentPort?: number;
        onSave: (config: SiteConfig) => Promise<void>;
        onRemove: () => Promise<void>;
        onRetryCertificate: () => Promise<void>;
        /** Abre el sitio en el navegador del sistema. */
        onOpenSite?: (url: string) => void;
    }

    let { site, publishDir, agentPort = 4040, onSave, onRemove, onRetryCertificate, onOpenSite }: Props = $props();

    const siteUrl = $derived(site ? `${site.tls === 'active' ? 'https' : 'http'}://${site.domains[0]}` : '');

    const initial = untrack(() => site);
    let template = $state<SiteTemplate>(initial?.template ?? 'app');
    let primary = $state(initial?.domains[0] ?? '');
    let aliases = $state(initial?.domains.slice(1).join(', ') ?? '');
    // El campo numérico entrega un número (o cadena vacía si se borra).
    let port = $state<number | string>(initial?.port ?? '');
    const portText = $derived(String(port).trim());
    let https = $state(initial?.https ?? true);
    let email = $state(initial?.email ?? '');
    let submitted = $state(false);
    let busy = $state(false);
    let retrying = $state(false);
    let removeOpen = $state(false);
    let formError = $state<string>();

    const templateOptions = $derived(
        SITE_TEMPLATES.map((value) => ({ value, label: t(`services.site.templates.${value}.label`) })),
    );
    const needsPort = $derived(proxies(template));
    const needsFiles = $derived(servesFiles(template));

    /** Mismas reglas que el agente (nginx::sites::validate). */
    const DOMAIN = /^(?=.{1,253}$)(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9-]*[a-z][a-z0-9-]*$/;
    const LOCAL = ['.localhost', '.local', '.test', '.internal', '.lan', '.home.arpa'];
    const normalize = (domain: string) => domain.trim().replace(/\.$/, '').toLowerCase();
    const domains = $derived([primary, ...aliases.split(',')].map(normalize).filter((domain) => domain !== ''));
    const validDomain = (domain: string) => domain === 'localhost' || DOMAIN.test(domain);
    const primaryError = $derived.by(() => {
        const domain = normalize(primary);
        if (!domain) return t('services.site.domainRequired');
        return validDomain(domain) ? undefined : t('services.site.domainInvalid');
    });
    const aliasesError = $derived.by(() => {
        const list = domains.slice(1);
        if (list.some((domain) => !validDomain(domain))) return t('services.site.domainInvalid');
        if (new Set(domains).size !== domains.length) return t('services.site.domainRepeated');
        return undefined;
    });
    const portError = $derived.by(() => {
        if (!needsPort) return undefined;
        const value = Number(portText);
        if (!portText || !Number.isInteger(value) || value < 1024 || value > 65535) {
            return t('services.site.portInvalid');
        }
        return value === agentPort ? t('services.site.portAgent') : undefined;
    });
    const httpsError = $derived(
        https && domains.some((domain) => domain === 'localhost' || LOCAL.some((suffix) => domain.endsWith(suffix)))
            ? t('services.site.httpsLocal')
            : undefined,
    );
    const emailError = $derived(
        https && email.trim() && !/^[A-Za-z0-9._%+-]{1,64}@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$/.test(email.trim())
            ? t('services.site.emailInvalid')
            : undefined,
    );

    const tlsBadge = $derived(
        site
            ? (
                  {
                      off: { variant: 'neutral', label: t('services.site.tls.off') },
                      issuing: { variant: 'info', label: t('services.site.tls.issuing') },
                      active: { variant: 'success', label: t('services.site.tls.active') },
                      failed: { variant: 'danger', label: t('services.site.tls.failed') },
                  } as const
              )[site.tls]
            : null,
    );

    async function save(event: SubmitEvent): Promise<void> {
        event.preventDefault();
        submitted = true;
        formError = undefined;
        if (primaryError || aliasesError || portError || httpsError || emailError) return;
        if (needsFiles && !publishDir) return;
        busy = true;
        try {
            await onSave({
                template,
                domains,
                port: needsPort ? Number(portText) : null,
                https,
                email: https && email.trim() ? email.trim() : null,
            });
            submitted = false;
        } catch (error) {
            formError = describeError(error);
        } finally {
            busy = false;
        }
    }

    async function retry(): Promise<void> {
        retrying = true;
        formError = undefined;
        try {
            await onRetryCertificate();
        } catch (error) {
            formError = describeError(error);
        } finally {
            retrying = false;
        }
    }
</script>

<form class="form" novalidate onsubmit={save}>
    {#if site && tlsBadge}
        <div class="status">
            <div class="status-row">
                <Badge variant={tlsBadge.variant} dot>{tlsBadge.label}</Badge>
                <code class="address">{siteUrl}</code>
                {#if onOpenSite}
                    <Button
                        size="sm"
                        variant="ghost"
                        iconOnly
                        aria-label={t('services.site.open')}
                        onclick={() => onOpenSite(siteUrl)}
                    >
                        <ExternalLinkIcon />
                    </Button>
                {/if}
            </div>
            {#if site.tls === 'failed'}
                {#if site.tlsDetail}<pre class="detail">{site.tlsDetail}</pre>{/if}
                <p class="note">{t('services.site.tlsFailedHint')}</p>
                <div>
                    <Button size="sm" loading={retrying} onclick={retry}>{t('services.site.retry')}</Button>
                </div>
            {:else if site.tls === 'issuing'}
                <p class="note">{t('services.site.tlsIssuingHint')}</p>
            {/if}
            <p class="note">{t('services.site.configPath')} <code>{site.configPath}</code></p>
        </div>
    {/if}

    <Select
        label={t('services.site.template')}
        hint={t(`services.site.templates.${template}.description`)}
        options={templateOptions}
        bind:value={template}
    />
    {#if needsFiles && !publishDir}
        <p class="warning" role="status">{t('services.site.needsPublishDir')}</p>
    {/if}

    <div class="row">
        <div class="grow">
            <Input
                label={t('services.site.domain')}
                placeholder={t('services.site.domainPlaceholder')}
                autocomplete="off"
                spellcheck="false"
                error={submitted ? primaryError : undefined}
                bind:value={primary}
            />
        </div>
        <div class="grow">
            <Input
                label={t('services.site.aliases')}
                hint={t('services.site.aliasesHint')}
                placeholder={t('services.site.aliasesPlaceholder')}
                autocomplete="off"
                spellcheck="false"
                error={submitted ? aliasesError : undefined}
                bind:value={aliases}
            />
        </div>
    </div>

    {#if needsPort}
        <Input
            label={t('services.site.port')}
            hint={t('services.site.portHint', { port: portText || '3000' })}
            type="number"
            min={1024}
            max={65535}
            placeholder="3000"
            error={submitted ? portError : undefined}
            bind:value={port}
        />
    {/if}

    <Switch label={t('services.site.https')} description={t('services.site.httpsHint')} bind:checked={https} />
    {#if submitted && httpsError}<p class="alert" role="alert">{httpsError}</p>{/if}
    {#if https}
        <Input
            label={t('services.site.email')}
            hint={t('services.site.emailHint')}
            type="email"
            autocomplete="email"
            placeholder={t('services.site.emailPlaceholder')}
            error={submitted ? emailError : undefined}
            bind:value={email}
        />
    {/if}

    {#if formError}<p class="alert" role="alert">{formError}</p>{/if}

    <div class="actions">
        {#if site}
            <Button variant="ghost" onclick={() => (removeOpen = true)}>
                <Trash2Icon />
                {t('services.site.remove')}
            </Button>
        {/if}
        <Button type="submit" variant="primary" loading={busy}>{t('services.site.save')}</Button>
    </div>
</form>

<ConfirmDialog
    bind:open={removeOpen}
    variant="danger"
    title={t('services.site.removeTitle')}
    description={t('services.site.removeDescription')}
    confirmLabel={t('services.site.remove')}
    onConfirm={onRemove}
/>

<style>
    .form {
        display: flex;
        flex-direction: column;
        gap: var(--space-5);
    }

    .status {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        padding: var(--space-3) var(--space-4);
        background-color: var(--color-bg-subtle);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
    }

    .status-row {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--space-3);
    }

    .address {
        font-size: var(--font-size-sm);
        overflow-wrap: anywhere;
    }

    .detail {
        max-height: var(--layout-card-min-width);
        padding: var(--space-2) var(--space-3);
        overflow: auto;
        font-family: var(--font-mono);
        font-size: var(--font-size-xs);
        white-space: pre-wrap;
        background-color: var(--color-surface);
        border-radius: var(--radius-sm);
    }

    .row {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-3);
    }

    .grow {
        flex: 1 1 min(100%, var(--layout-card-min-width));
        min-width: 0;
    }

    .note {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
        overflow-wrap: anywhere;
    }

    .warning {
        padding: var(--space-3);
        font-size: var(--font-size-sm);
        background-color: var(--color-warning-subtle);
        border-radius: var(--radius-md);
    }

    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }

    .actions {
        display: flex;
        flex-wrap: wrap;
        justify-content: flex-end;
        gap: var(--space-2);
    }
</style>
