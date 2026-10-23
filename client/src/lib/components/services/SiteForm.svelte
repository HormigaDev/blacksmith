<!--
    Sitio web del servicio detrás de nginx: tipo de proyecto (plantilla), dominios, puerto del
    servicio y HTTPS con Let's Encrypt. Valida como el agente antes de guardar; el agente genera la
    configuración, la prueba con `nginx -t` y, si nginx la rechaza, mantiene la anterior.

    Rutas adicionales: prefijos que nginx reenvía a otros puertos del servidor antes que la
    plantilla (p. ej. /api/ → 3000 y /socket.io/ → 3001 con WebSocket), para proyectos con varios
    procesos. El agente rechaza las que choquen con la plantilla.

    Estado del certificado: se pide en segundo plano tras guardar (el sitio ya responde por HTTP);
    si falla, se muestra el motivo y se puede reintentar.
-->
<script lang="ts">
    import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
    import ExternalLinkIcon from '@lucide/svelte/icons/external-link';
    import PlusIcon from '@lucide/svelte/icons/plus';
    import RouteIcon from '@lucide/svelte/icons/route';
    import Trash2Icon from '@lucide/svelte/icons/trash-2';
    import XIcon from '@lucide/svelte/icons/x';
    import { untrack } from 'svelte';

    import {
        MAX_PRIMARY_DOMAIN_LENGTH,
        MAX_SITE_DOMAINS,
        proxies,
        servesFiles,
        type Site,
        type SiteConfig,
        SITE_TEMPLATES,
        type SiteTemplate,
        validDomain,
        validEmail,
    } from '$lib/agent';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';

    import Badge from '../ui/Badge.svelte';
    import Button from '../ui/Button.svelte';
    import Checkbox from '../ui/Checkbox.svelte';
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

    const uid = $props.id();
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
    interface RouteRow {
        key: number;
        path: string;
        port: number | string;
        websocket: boolean;
    }
    let nextKey = 0;
    let routes = $state<RouteRow[]>((initial?.routes ?? []).map((route) => ({ key: nextKey++, ...route })));
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
    const LOCAL = ['.localhost', '.local', '.test', '.internal', '.lan', '.home.arpa'];
    const normalize = (domain: string) => domain.trim().replace(/\.$/, '').toLowerCase();
    const domains = $derived([primary, ...aliases.split(',')].map(normalize).filter((domain) => domain !== ''));
    const primaryError = $derived.by(() => {
        const domain = normalize(primary);
        if (!domain) return t('services.site.domainRequired');
        if (!validDomain(domain)) return t('services.site.domainInvalid');
        return domain.length > MAX_PRIMARY_DOMAIN_LENGTH
            ? t('services.site.domainTooLong', { max: MAX_PRIMARY_DOMAIN_LENGTH })
            : undefined;
    });
    const aliasesError = $derived.by(() => {
        const list = domains.slice(1);
        if (list.some((domain) => !validDomain(domain))) return t('services.site.domainInvalid');
        if (new Set(domains).size !== domains.length) return t('services.site.domainRepeated');
        if (domains.length > MAX_SITE_DOMAINS) return t('services.site.domainTooMany', { max: MAX_SITE_DOMAINS });
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
    /** Mismas reglas que el agente (nginx::sites::validate_routes). */
    const MAX_ROUTES = 20;
    const ROUTE = /^\/[A-Za-z0-9/._~-]{1,99}$/;
    /** Prefijos que ya usan las plantillas por defecto (el agente comprueba la plantilla real). */
    const TEMPLATE_LOCATIONS: Partial<Record<SiteTemplate, string[]>> = {
        spa: ['/index.html'],
        fullstack: ['/api/', '/ws', '/index.html'],
    };
    const routePath = (route: RouteRow) => route.path.trim();
    const routePort = (route: RouteRow) => String(route.port).trim();
    const pathError = (route: RouteRow, index: number) => {
        const path = routePath(route);
        if (
            !ROUTE.test(path) ||
            path.includes('//') ||
            path.split('/').some((part) => part === '.' || part === '..') ||
            path.startsWith('/.well-known')
        ) {
            return t('services.site.routes.pathInvalid');
        }
        if (routes.slice(0, index).some((other) => routePath(other) === path)) {
            return t('services.site.routes.pathRepeated');
        }
        if (TEMPLATE_LOCATIONS[template]?.includes(path)) return t('services.site.routes.pathTemplate');
        return undefined;
    };
    const routePortError = (route: RouteRow) => {
        const text = routePort(route);
        const value = Number(text);
        if (!text || !Number.isInteger(value) || value < 1024 || value > 65535) return t('services.site.portInvalid');
        return value === agentPort ? t('services.site.portAgent') : undefined;
    };
    const routesInvalid = $derived(
        routes.some((route, index) => pathError(route, index) !== undefined || routePortError(route) !== undefined),
    );

    function addRoute(): void {
        routes.push({ key: nextKey++, path: '', port: '', websocket: false });
    }

    function removeRoute(key: number): void {
        routes = routes.filter((route) => route.key !== key);
    }

    const httpsError = $derived(
        https && domains.some((domain) => domain === 'localhost' || LOCAL.some((suffix) => domain.endsWith(suffix)))
            ? t('services.site.httpsLocal')
            : undefined,
    );
    const emailError = $derived(
        https && email.trim() && !validEmail(email.trim()) ? t('services.site.emailInvalid') : undefined,
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
        if (primaryError || aliasesError || portError || routesInvalid || httpsError || emailError) return;
        if (needsFiles && !publishDir) return;
        busy = true;
        try {
            await onSave({
                template,
                domains,
                port: needsPort ? Number(portText) : null,
                https,
                email: https && email.trim() ? email.trim() : null,
                routes: routes.map((route) => ({
                    path: routePath(route),
                    port: Number(routePort(route)),
                    websocket: route.websocket,
                })),
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

    <section class="routes" aria-labelledby="{uid}-routes">
        <div class="routes-head">
            <RouteIcon />
            <div>
                <h4 id="{uid}-routes" class="routes-title">{t('services.site.routes.title')}</h4>
                <p class="note">{t('services.site.routes.hint')}</p>
            </div>
        </div>
        {#if routes.length > 0}
            <ul class="route-list">
                {#each routes as route, index (route.key)}
                    {@const number = index + 1}
                    <li class="route">
                        <div class="route-path">
                            <Input
                                label={t('services.site.routes.path', { number })}
                                hideLabel
                                placeholder={t('services.site.routes.pathPlaceholder')}
                                autocomplete="off"
                                spellcheck="false"
                                error={submitted ? pathError(route, index) : undefined}
                                bind:value={route.path}
                            >
                                {#snippet leading()}<RouteIcon />{/snippet}
                            </Input>
                        </div>
                        <span class="arrow" aria-hidden="true"><ArrowRightIcon /></span>
                        <div class="route-port">
                            <Input
                                label={t('services.site.routes.port', { number })}
                                hideLabel
                                type="number"
                                min={1024}
                                max={65535}
                                placeholder="3001"
                                error={submitted ? routePortError(route) : undefined}
                                bind:value={route.port}
                            />
                        </div>
                        <div class="centered">
                            <Checkbox label={t('services.site.routes.websocket')} bind:checked={route.websocket} />
                        </div>
                        <div class="centered">
                            <Button
                                size="sm"
                                variant="ghost"
                                iconOnly
                                aria-label={t('services.site.routes.remove', { number })}
                                onclick={() => removeRoute(route.key)}
                            >
                                <XIcon />
                            </Button>
                        </div>
                    </li>
                {/each}
            </ul>
        {/if}
        {#if routes.length < MAX_ROUTES}
            <div>
                <Button size="sm" onclick={addRoute}>
                    <PlusIcon />
                    {t('services.site.routes.add')}
                </Button>
            </div>
        {/if}
    </section>

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

    .routes {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
        padding: var(--space-3) var(--space-4);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
    }

    .routes-head {
        display: flex;
        gap: var(--space-3);
        align-items: flex-start;
    }

    .routes-head > :global(svg) {
        flex-shrink: 0;
        width: var(--icon-size-md);
        height: var(--icon-size-md);
        margin-top: var(--space-px);
        color: var(--color-accent);
    }

    .routes-title {
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-semibold);
    }

    .route-list {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        padding: var(--space-0);
        margin: var(--space-0);
        list-style: none;
    }

    .route {
        display: flex;
        flex-wrap: wrap;
        align-items: flex-start;
        gap: var(--space-2) var(--space-3);
    }

    .route-path {
        flex: 1 1 min(100%, var(--layout-tile-min-width));
        min-width: 0;
    }

    .route-port {
        flex: 0 1 var(--layout-tile-min-width);
        min-width: 0;
    }

    .arrow,
    .centered {
        display: flex;
        align-items: center;
        min-height: var(--control-height-md);
    }

    .arrow {
        color: var(--color-text-subtle);
    }

    .arrow :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
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
