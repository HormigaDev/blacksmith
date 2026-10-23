<!--
    Definición del despliegue de un servicio: repositorio y rama de GitHub, comandos de preparación
    (en orden) y comando de arranque. Valida como el agente antes de guardar.

    El repositorio, una vez guardado, no se puede cambiar (el agente también lo impide): solo la
    rama. El directorio de trabajo lo asigna el agente.

    Acceso: con la cuenta de GitHub del servidor (por defecto) o con una llave de despliegue del
    repositorio (más aislado; se gestiona en su propio panel).
-->
<script lang="ts">
    import { faGithub } from '@fortawesome/free-brands-svg-icons';
    import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
    import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
    import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
    import FolderIcon from '@lucide/svelte/icons/folder';
    import FolderGit2Icon from '@lucide/svelte/icons/folder-git-2';
    import FolderOpenIcon from '@lucide/svelte/icons/folder-open';
    import GitBranchIcon from '@lucide/svelte/icons/git-branch';
    import GlobeIcon from '@lucide/svelte/icons/globe';
    import KeyRoundIcon from '@lucide/svelte/icons/key-round';
    import LockIcon from '@lucide/svelte/icons/lock';
    import PlayIcon from '@lucide/svelte/icons/play';
    import PlusIcon from '@lucide/svelte/icons/plus';
    import SquareTerminalIcon from '@lucide/svelte/icons/square-terminal';
    import Trash2Icon from '@lucide/svelte/icons/trash-2';
    import UserRoundIcon from '@lucide/svelte/icons/user-round';
    import { type Component, untrack } from 'svelte';

    import type { RepositoryAccess, RepositoryRef, ServiceDeployment, UpdateDeployment } from '$lib/agent';
    import type { GitHubRepository } from '$lib/github';
    import { describeError } from '$lib/http';
    import { t } from '$lib/i18n';

    import Button from '../ui/Button.svelte';
    import FaIcon from '../ui/FaIcon.svelte';
    import Input from '../ui/Input.svelte';
    import FilterSelect from '../ui/FilterSelect.svelte';
    import Spinner from '../ui/Spinner.svelte';
    import ToggleGroup from '../ui/ToggleGroup.svelte';

    interface Props {
        deployment: ServiceDeployment;
        /** `false` si no hay cuenta de GitHub conectada. */
        githubConnected: boolean;
        /** Repositorios accesibles; `null` mientras cargan. */
        repositories: readonly GitHubRepository[] | null;
        loadBranches: (owner: string, name: string) => Promise<string[]>;
        onConnectGitHub?: () => void;
        /** Guarda la definición. Si lanza, el error se muestra aquí. */
        onSave: (update: UpdateDeployment) => Promise<void>;
    }

    let { deployment, githubConnected, repositories, loadBranches, onConnectGitHub, onSave }: Props = $props();

    const NONE = '';
    const MAX_COMMANDS = 20;
    const initial = untrack(() => deployment);
    /** Repositorio guardado: ya no se puede cambiar. */
    const locked = $derived(deployment.repository);
    let repository = $state(initial.repository ? `${initial.repository.owner}/${initial.repository.name}` : NONE);
    let branch = $state(initial.repository?.branch ?? '');
    let branches = $state<string[]>(initial.repository ? [initial.repository.branch] : []);
    let branchesLoading = $state(false);
    let nextId = 0;
    let commands = $state(initial.setupCommands.map((value) => ({ id: nextId++, value })));
    const sectionId = $props.id();
    let startCommand = $state(initial.startCommand);
    let publishDir = $state(initial.publishDir ?? '');
    let access = $state<RepositoryAccess>(initial.access);
    const accessOptions = $derived([
        { value: 'account' as const, label: t('services.deployment.accessAccount'), icon: UserRoundIcon },
        { value: 'deployKey' as const, label: t('services.deployment.accessDeployKey'), icon: KeyRoundIcon },
    ]);
    let busy = $state(false);
    let submitted = $state(false);
    let formError = $state<string>();

    // eslint-disable-next-line no-control-regex
    const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;

    const repositoryOptions = $derived([
        { value: NONE, label: t('services.deployment.repositoryNone') },
        ...(repositories ?? []).map((repo) => ({ value: repo.fullName, label: repo.fullName })),
    ]);
    const branchOptions = $derived(branches.map((name) => ({ value: name, label: name })));
    const commandErrors = $derived(
        commands.map(({ value }) =>
            !value.trim()
                ? t('services.deployment.commandEmpty')
                : CONTROL.test(value)
                  ? t('services.deployment.commandInvalid')
                  : undefined,
        ),
    );
    const startError = $derived(CONTROL.test(startCommand) ? t('services.deployment.commandInvalid') : undefined);
    /** Misma regla que el agente: ruta relativa dentro del repositorio, sin `..`. */
    const publishDirError = $derived.by(() => {
        const dir = publishDir.trim().replace(/\/+$/, '');
        if (dir === '' || dir === '.') return undefined;
        const valid =
            dir.length <= 200 &&
            /^[A-Za-z0-9._/-]+$/.test(dir) &&
            !dir.startsWith('/') &&
            dir.split('/').every((part) => part !== '' && part !== '.' && part !== '..');
        return valid ? undefined : t('services.deployment.publishDirInvalid');
    });

    async function fetchBranches(owner: string, name: string, preferred?: string): Promise<void> {
        branchesLoading = true;
        try {
            const loaded = await loadBranches(owner, name);
            // La rama guardada sigue elegible aunque ya no exista en GitHub.
            branches = branch && !loaded.includes(branch) ? [branch, ...loaded] : loaded;
            if (!branch) branch = preferred && loaded.includes(preferred) ? preferred : (loaded[0] ?? '');
        } catch (error) {
            formError = describeError(error);
        } finally {
            branchesLoading = false;
        }
    }

    // Con el repositorio ya fijado, las ramas se cargan en cuanto hay GitHub.
    $effect(() => {
        const repo = locked;
        if (repo && githubConnected) untrack(() => void fetchBranches(repo.owner, repo.name));
    });

    async function selectRepository(fullName: string): Promise<void> {
        repository = fullName;
        branch = '';
        branches = [];
        if (fullName === NONE) return;
        const [owner = '', name = ''] = fullName.split('/');
        const known = repositories?.find((repo) => repo.fullName === fullName);
        await fetchBranches(owner, name, known?.defaultBranch);
    }

    function move(index: number, offset: -1 | 1): void {
        const item = commands[index];
        const other = commands[index + offset];
        if (!item || !other) return;
        commands[index] = other;
        commands[index + offset] = item;
    }

    async function save(event: SubmitEvent): Promise<void> {
        event.preventDefault();
        submitted = true;
        formError = undefined;
        if (commandErrors.some(Boolean) || startError || publishDirError) return;
        let repo: RepositoryRef | null = null;
        if (repository !== NONE) {
            const [owner = '', name = ''] = repository.split('/');
            repo = { owner, name, branch };
        }
        busy = true;
        try {
            await onSave({
                repository: repo,
                access,
                setupCommands: commands.map(({ value }) => value.trim()),
                startCommand: startCommand.trim(),
                publishDir: publishDir.trim().replace(/\/+$/, '') || null,
            });
            submitted = false;
        } catch (error) {
            formError = describeError(error);
        } finally {
            busy = false;
        }
    }
</script>

{#snippet header(key: string, Icon: Component, title: string, hint: string)}
    <header class="section-header">
        <span class="section-icon" aria-hidden="true"><Icon /></span>
        <div>
            <h3 class="section-title" id="{sectionId}-{key}">{title}</h3>
            <p class="section-hint">{hint}</p>
        </div>
    </header>
{/snippet}

<form class="form" novalidate onsubmit={save}>
    <section class="section" aria-labelledby="{sectionId}-source">
        {@render header(
            'source',
            FolderGit2Icon,
            t('services.deployment.sectionSource'),
            t('services.deployment.sectionSourceHint'),
        )}
        <div class="row">
            <div class="grow">
                {#if locked}
                    <div class="readonly">
                        <span class="label">{t('services.deployment.repository')}</span>
                        <span class="field-box">
                            <FaIcon icon={faGithub} />
                            <span class="field-value" data-selectable>{locked.owner}/{locked.name}</span>
                            <span
                                class="lock"
                                role="img"
                                title={t('services.deployment.repositoryLocked')}
                                aria-label={t('services.deployment.repositoryLocked')}
                            >
                                <LockIcon aria-hidden="true" />
                            </span>
                        </span>
                    </div>
                {:else if !githubConnected}
                    <div class="readonly">
                        <span class="label">{t('services.deployment.repository')}</span>
                        {#if onConnectGitHub}
                            <div><Button onclick={onConnectGitHub}>{t('github.connect')}</Button></div>
                        {/if}
                    </div>
                {:else if repositories === null}
                    <div class="readonly">
                        <span class="label">{t('services.deployment.repository')}</span>
                        <Spinner label={t('services.deployment.repositoriesLoading')} />
                    </div>
                {:else}
                    <FilterSelect
                        label={t('services.deployment.repository')}
                        hint={t('services.deployment.repositoryHint')}
                        placeholder={t('services.deployment.repositoryPlaceholder')}
                        searchPlaceholder={t('services.deployment.repositorySearch')}
                        options={repositoryOptions}
                        value={repository}
                        onValueChange={selectRepository}
                    />
                {/if}
            </div>
            {#if repository !== NONE}
                <div class="branch">
                    {#if githubConnected}
                        <FilterSelect
                            label={t('services.deployment.branch')}
                            placeholder={branchesLoading ? t('services.deployment.branchLoading') : undefined}
                            searchPlaceholder={t('services.deployment.branchSearch')}
                            options={branchOptions}
                            disabled={branchesLoading}
                            bind:value={branch}
                        >
                            {#snippet leading()}<GitBranchIcon />{/snippet}
                        </FilterSelect>
                    {:else}
                        <Input label={t('services.deployment.branch')} spellcheck="false" bind:value={branch}>
                            {#snippet leading()}<GitBranchIcon />{/snippet}
                        </Input>
                    {/if}
                </div>
            {/if}
        </div>

        <div class="group">
            <span class="label">{t('services.deployment.access')}</span>
            <div>
                <ToggleGroup
                    label={t('services.deployment.access')}
                    options={accessOptions}
                    size="sm"
                    bind:value={access}
                />
            </div>
            <p class="hint">
                {access === 'account'
                    ? t('services.deployment.accessAccountHint')
                    : t('services.deployment.accessDeployKeyHint')}
            </p>
        </div>
    </section>

    <section class="section" aria-labelledby="{sectionId}-run">
        {@render header(
            'run',
            SquareTerminalIcon,
            t('services.deployment.sectionRun'),
            t('services.deployment.sectionRunHint'),
        )}
        <div class="readonly">
            <span class="label">{t('services.deployment.workingDirectory')}</span>
            <span class="field-box">
                <FolderIcon />
                <code class="field-value">{deployment.workingDirectory}</code>
            </span>
        </div>

        <div class="group">
            <span class="label">{t('services.deployment.setupTitle')}</span>
            {#if commands.length > 0}
                <ol class="commands">
                    {#each commands as command, index (command.id)}
                        <li class="command">
                            <span class="step">{index + 1}</span>
                            <div class="grow">
                                <Input
                                    label={t('services.deployment.command', { index: index + 1 })}
                                    hideLabel
                                    placeholder={t('services.deployment.commandPlaceholder')}
                                    autocomplete="off"
                                    spellcheck="false"
                                    maxlength={2048}
                                    error={submitted ? commandErrors[index] : undefined}
                                    bind:value={command.value}
                                >
                                    {#snippet leading()}<ChevronRightIcon />{/snippet}
                                </Input>
                            </div>
                            <Button
                                variant="ghost"
                                size="sm"
                                iconOnly
                                disabled={index === 0}
                                aria-label={t('services.deployment.moveUp', { index: index + 1 })}
                                onclick={() => move(index, -1)}
                            >
                                <ArrowUpIcon />
                            </Button>
                            <Button
                                variant="ghost"
                                size="sm"
                                iconOnly
                                disabled={index === commands.length - 1}
                                aria-label={t('services.deployment.moveDown', { index: index + 1 })}
                                onclick={() => move(index, 1)}
                            >
                                <ArrowDownIcon />
                            </Button>
                            <Button
                                variant="ghost"
                                size="sm"
                                iconOnly
                                aria-label={t('services.deployment.removeCommand', { index: index + 1 })}
                                onclick={() => (commands = commands.filter((other) => other.id !== command.id))}
                            >
                                <Trash2Icon />
                            </Button>
                        </li>
                    {/each}
                </ol>
            {/if}
            <div>
                <Button
                    size="sm"
                    variant="ghost"
                    disabled={commands.length >= MAX_COMMANDS}
                    onclick={() => commands.push({ id: nextId++, value: '' })}
                >
                    <PlusIcon />
                    {t('services.deployment.addCommand')}
                </Button>
            </div>
        </div>

        <Input
            label={t('services.deployment.startCommand')}
            hint={t('services.deployment.startCommandHint')}
            placeholder={t('services.deployment.startCommandPlaceholder')}
            autocomplete="off"
            spellcheck="false"
            maxlength={1024}
            error={submitted ? startError : undefined}
            bind:value={startCommand}
        >
            {#snippet leading()}<PlayIcon />{/snippet}
        </Input>
    </section>

    <section class="section" aria-labelledby="{sectionId}-publish">
        {@render header(
            'publish',
            GlobeIcon,
            t('services.deployment.sectionPublish'),
            t('services.deployment.sectionPublishHint'),
        )}
        <Input
            label={t('services.deployment.publishDir')}
            hint={t('services.deployment.publishDirHint')}
            placeholder={t('services.deployment.publishDirPlaceholder')}
            autocomplete="off"
            spellcheck="false"
            maxlength={200}
            error={submitted ? publishDirError : undefined}
            bind:value={publishDir}
        >
            {#snippet leading()}<FolderOpenIcon />{/snippet}
        </Input>
    </section>

    {#if formError}<p class="alert" role="alert">{formError}</p>{/if}

    <div class="actions">
        <Button type="submit" variant="primary" loading={busy}>{t('services.deployment.save')}</Button>
    </div>
</form>

<style>
    .form {
        display: flex;
        flex-direction: column;
        gap: var(--space-6);
    }

    /* Secciones separadas por un divisor: origen, ejecución y publicación. */
    .section {
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
    }

    .section + .section {
        padding-top: var(--space-6);
        border-top: var(--border-width) solid var(--color-border);
    }

    .section-header {
        display: flex;
        gap: var(--space-3);
        align-items: flex-start;
    }

    /* Icono de la sección: un recuadro del color de acento, el ancla visual de cada bloque. */
    .section-icon {
        display: inline-flex;
        flex-shrink: 0;
        align-items: center;
        justify-content: center;
        width: var(--control-height-md);
        height: var(--control-height-md);
        color: var(--color-accent);
        background-color: var(--color-accent-subtle);
        border-radius: var(--radius-md);
    }

    .section-icon :global(svg) {
        width: var(--icon-size-md);
        height: var(--icon-size-md);
    }

    .section-title {
        font-size: var(--font-size-md);
        font-weight: var(--font-weight-semibold);
        color: var(--color-text);
    }

    .section-hint {
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    /* Valor de solo lectura con aspecto de campo (repositorio fijado, directorio de trabajo). */
    .field-box {
        display: flex;
        gap: var(--space-2);
        align-items: center;
        min-width: 0;
        min-height: var(--control-height-md);
        padding: var(--space-0) var(--space-3);
        font-size: var(--font-size-sm);
        color: var(--color-text);
        background-color: var(--color-bg-subtle);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
    }

    .field-box > :global(svg),
    .lock :global(svg) {
        flex-shrink: 0;
        width: var(--icon-size-sm);
        height: var(--icon-size-sm);
        color: var(--color-text-muted);
    }

    .field-value {
        min-width: 0;
        overflow: hidden;
        font-family: var(--font-mono);
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .lock {
        display: inline-flex;
        margin-left: auto;
    }

    .row {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-3);
    }

    .grow {
        flex: 1;
        min-width: 0;
    }

    /* Repositorio y rama comparten fila mientras quepan; si no, la rama baja. */
    .row > .grow {
        flex-basis: min(100%, var(--layout-card-min-width));
    }

    .branch {
        width: min(100%, var(--layout-card-min-width));
    }

    .group {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
    }

    /* Mismo espacio entre etiqueta y valor que los campos (Field): quedan alineados en fila. */
    .readonly {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
    }

    .label {
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-medium);
    }

    .commands {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        padding: var(--space-0);
        margin: var(--space-0);
        list-style: none;
    }

    .command {
        display: flex;
        align-items: flex-start;
        gap: var(--space-1);
    }

    .step {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: var(--control-height-md);
        height: var(--control-height-md);
        font-family: var(--font-mono);
        font-size: var(--font-size-xs);
        color: var(--color-text-subtle);
    }

    .hint {
        font-size: var(--font-size-xs);
        color: var(--color-text-muted);
    }

    .alert {
        font-size: var(--font-size-sm);
        color: var(--color-danger);
    }

    .actions {
        display: flex;
        justify-content: flex-end;
    }
</style>
