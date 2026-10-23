<script lang="ts" module>
    import type { StoryMeta } from '$lib/workbench';

    export const meta: StoryMeta = {
        title: 'SiteForm',
        group: 'Servicios',
        description: 'Sitio web del servicio en nginx: plantilla, dominios, puerto y HTTPS.',
    };
</script>

<script lang="ts">
    import type { Site } from '$lib/agent';
    import { Story } from '$lib/workbench';

    import SiteForm from '../SiteForm.svelte';

    const site: Site = {
        name: 'web',
        template: 'fullstack',
        domains: ['app.example.com', 'www.app.example.com'],
        port: 3000,
        https: true,
        email: 'ops@example.com',
        tls: 'active',
        tlsDetail: null,
        configPath: '/var/lib/blacksmith/nginx/sites/web.conf',
        updatedAt: 1,
    };
    const handlers = {
        onSave: async () => {},
        onRemove: async () => {},
        onRetryCertificate: async () => {},
        onOpenSite: () => {},
    };
</script>

<Story name="Nuevo" layout="stack">
    <div class="frame"><SiteForm site={null} publishDir={null} {...handlers} /></div>
</Story>

<Story name="HTTPS activo" layout="stack">
    <div class="frame"><SiteForm {site} publishDir="dist" {...handlers} /></div>
</Story>

<Story name="Pidiendo certificado" layout="stack">
    <div class="frame"><SiteForm site={{ ...site, tls: 'issuing' }} publishDir="dist" {...handlers} /></div>
</Story>

<Story name="Certificado fallido" layout="stack">
    <div class="frame">
        <SiteForm
            site={{
                ...site,
                tls: 'failed',
                tlsDetail:
                    'Domain: app.example.com\nType: connection\nDetail: Timeout during connect (likely firewall problem)',
            }}
            publishDir="dist"
            {...handlers}
        />
    </div>
</Story>

<Story name="Sin carpeta pública" layout="stack">
    <div class="frame"><SiteForm site={{ ...site, https: false, tls: 'off' }} publishDir={null} {...handlers} /></div>
</Story>

<style>
    .frame {
        width: min(100%, var(--dialog-width-lg));
    }
</style>
