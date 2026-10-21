<!--
    Selector de archivos o carpetas. El valor son RUTAS absolutas (lo que necesita el backend,
    p. ej. una llave SSH), no objetos `File`.
    - En Tauri usa el diálogo nativo y el arrastrar y soltar de la ventana (Tauri intercepta el
      drop de HTML5 y entrega las rutas con la posición, que aquí se compara con la zona).
    - En el navegador (`pnpm dev`) cae a <input type="file">, que solo conoce el nombre: es un
      modo de desarrollo, no de uso real.
-->
<script lang="ts">
    import FileIcon from '@lucide/svelte/icons/file';
    import FolderIcon from '@lucide/svelte/icons/folder';
    import UploadIcon from '@lucide/svelte/icons/upload';
    import XIcon from '@lucide/svelte/icons/x';
    import { isTauri } from '@tauri-apps/api/core';
    import type { PhysicalPosition } from '@tauri-apps/api/dpi';
    import { getCurrentWebview } from '@tauri-apps/api/webview';
    import { open } from '@tauri-apps/plugin-dialog';

    import { formatList, t } from '$lib/i18n';

    import Button from './Button.svelte';
    import Field from './Field.svelte';

    interface Props {
        label: string;
        hideLabel?: boolean;
        hint?: string;
        error?: string;
        required?: boolean;
        /** Rutas seleccionadas. */
        value?: string[];
        multiple?: boolean;
        /** Seleccionar carpetas en vez de archivos. */
        directory?: boolean;
        /** Extensiones admitidas, sin punto: `['pem', 'key']`. */
        accept?: readonly string[];
        disabled?: boolean;
        onValueChange?: (value: string[]) => void;
    }

    let {
        label,
        hideLabel = false,
        hint,
        error,
        required = false,
        value = $bindable([]),
        multiple = false,
        directory = false,
        accept,
        disabled = false,
        onValueChange,
    }: Props = $props();

    let zone = $state<HTMLElement>();
    let fallbackInput = $state<HTMLInputElement>();
    let dragging = $state(false);
    let rejected = $state(0);

    const extensions = $derived(directory ? [] : (accept ?? []).map((extension) => extension.toLowerCase()));

    const baseName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;
    const parentDir = (path: string) => path.slice(0, Math.max(0, path.length - baseName(path).length - 1));

    function accepts(path: string): boolean {
        if (extensions.length === 0) return true;
        const name = baseName(path).toLowerCase();
        return extensions.some((extension) => name.endsWith(`.${extension}`));
    }

    function add(paths: readonly string[]): void {
        const valid = paths.filter(accepts);
        rejected = paths.length - valid.length;
        if (valid.length === 0) return;
        value = multiple ? [...new Set([...value, ...valid])] : [valid[0] as string];
        onValueChange?.(value);
    }

    function remove(path: string): void {
        value = value.filter((item) => item !== path);
        rejected = 0;
        onValueChange?.(value);
    }

    async function browse(): Promise<void> {
        if (!isTauri()) {
            fallbackInput?.click();
            return;
        }
        const selection = await open({
            multiple,
            directory,
            filters: extensions.length > 0 ? [{ name: label, extensions }] : undefined,
        });
        if (selection === null) return;
        add(Array.isArray(selection) ? selection : [selection]);
    }

    function onFallbackChange(event: Event & { currentTarget: HTMLInputElement }): void {
        add([...(event.currentTarget.files ?? [])].map((file) => file.name));
        event.currentTarget.value = '';
    }

    function isOverZone(position: PhysicalPosition): boolean {
        if (!zone) return false;
        const scale = window.devicePixelRatio || 1;
        const [x, y] = [position.x / scale, position.y / scale];
        const rect = zone.getBoundingClientRect();
        return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
    }

    $effect(() => {
        if (!isTauri() || disabled) return;
        let unlisten: (() => void) | undefined;
        let disposed = false;
        void getCurrentWebview()
            .onDragDropEvent(({ payload }) => {
                if (payload.type === 'leave') {
                    dragging = false;
                } else if (payload.type === 'drop') {
                    dragging = false;
                    if (isOverZone(payload.position)) add(payload.paths);
                } else {
                    dragging = isOverZone(payload.position);
                }
            })
            .then((stop) => {
                if (disposed) stop();
                else unlisten = stop;
            });
        return () => {
            disposed = true;
            unlisten?.();
        };
    });
</script>

<Field {label} {hideLabel} {hint} {error} {required}>
    {#snippet children({ id, describedBy, invalid })}
        <div
            bind:this={zone}
            class="zone"
            class:dragging
            class:invalid
            class:disabled
            data-invalid={invalid || undefined}
        >
            <span class="zone-icon"><UploadIcon /></span>
            <p class="zone-title">
                {#if dragging}
                    {t('components.filePicker.dropActive')}
                {:else if directory}
                    {t('components.filePicker.dropDirectory')}
                {:else}
                    {multiple ? t('components.filePicker.dropFiles') : t('components.filePicker.dropFile')}
                {/if}
            </p>
            <Button {id} size="sm" {disabled} aria-describedby={describedBy} onclick={browse}>
                {directory
                    ? t('components.filePicker.browseDirectory')
                    : multiple
                      ? t('components.filePicker.browseFiles')
                      : t('components.filePicker.browseFile')}
            </Button>
            {#if extensions.length > 0}
                <p class="zone-formats">
                    {t('components.filePicker.formats', {
                        formats: formatList(
                            extensions.map((extension) => `.${extension}`),
                            'unit',
                        ),
                    })}
                </p>
            {/if}
            <input
                bind:this={fallbackInput}
                class="bs-visually-hidden"
                type="file"
                tabindex="-1"
                aria-hidden="true"
                {multiple}
                accept={extensions.map((extension) => `.${extension}`).join(',') || undefined}
                onchange={onFallbackChange}
            />
        </div>

        {#if rejected > 0}
            <p class="rejected" role="alert">{t('components.filePicker.rejected', { count: rejected })}</p>
        {/if}

        {#if value.length > 0}
            <ul class="files">
                {#each value as path (path)}
                    <li class="file">
                        <span class="file-icon">
                            {#if directory}<FolderIcon />{:else}<FileIcon />{/if}
                        </span>
                        <span class="file-text">
                            <span class="file-name">{baseName(path)}</span>
                            {#if parentDir(path)}
                                <span class="file-path">{parentDir(path)}</span>
                            {/if}
                        </span>
                        <button
                            type="button"
                            class="bs-icon-button"
                            aria-label={t('components.filePicker.remove', { name: baseName(path) })}
                            {disabled}
                            onclick={() => remove(path)}
                        >
                            <XIcon />
                        </button>
                    </li>
                {/each}
            </ul>
        {/if}
    {/snippet}
</Field>

<style>
    .zone {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-6) var(--space-4);
        text-align: center;
        background-color: var(--color-bg-subtle);
        border: var(--border-width-strong) dashed var(--color-border-strong);
        border-radius: var(--radius-lg);
        transition:
            border-color var(--duration-fast) var(--easing-standard),
            background-color var(--duration-fast) var(--easing-standard);
    }

    .zone.dragging {
        background-color: var(--color-accent-subtle);
        border-color: var(--color-accent);
    }

    .zone.invalid {
        border-color: var(--color-danger);
    }

    .zone.disabled {
        opacity: 0.6;
    }

    .zone-icon {
        display: inline-flex;
        color: var(--color-text-subtle);
    }

    .dragging .zone-icon {
        color: var(--color-accent);
    }

    .zone-icon :global(svg) {
        width: var(--icon-size-lg);
        height: var(--icon-size-lg);
    }

    .zone-title {
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-medium);
    }

    .zone-formats {
        font-size: var(--font-size-xs);
        color: var(--color-text-subtle);
    }

    .rejected {
        font-size: var(--font-size-xs);
        color: var(--color-danger);
    }

    .files {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
        padding: var(--space-0);
        list-style: none;
    }

    .file {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-2) var(--space-3);
        background-color: var(--color-surface);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
    }

    .file-icon {
        display: inline-flex;
        color: var(--color-text-subtle);
    }

    .file-icon :global(svg) {
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
    }

    .file-text {
        display: flex;
        flex: 1;
        flex-direction: column;
        min-width: 0;
    }

    .file-name,
    .file-path {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .file-name {
        font-size: var(--font-size-sm);
    }

    .file-path {
        font-family: var(--font-mono);
        font-size: var(--font-size-xs);
        color: var(--color-text-subtle);
    }
</style>
