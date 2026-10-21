<!-- Selector nativo con etiqueta. `T` es el tipo de los valores de las opciones. -->
<script lang="ts" generics="T extends string">
    interface Option {
        value: T;
        label: string;
    }

    interface Props {
        label: string;
        options: readonly Option[];
        value: T;
    }

    let { label, options, value = $bindable() }: Props = $props();
</script>

<label class="field">
    <span class="label">{label}</span>
    <select class="select" bind:value>
        {#each options as option (option.value)}
            <option value={option.value}>{option.label}</option>
        {/each}
    </select>
</label>

<style>
    .field {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
        font-size: var(--font-size-sm);
        color: var(--color-text-muted);
    }

    .select {
        height: var(--control-height-sm);
        padding: var(--space-0) var(--space-2);
        border: var(--border-width) solid var(--color-border);
        border-radius: var(--radius-md);
        background-color: var(--color-surface);
        color: var(--color-text);
        transition: border-color var(--duration-fast) var(--easing-standard);
    }

    .select:hover {
        border-color: var(--color-border-strong);
    }
</style>
