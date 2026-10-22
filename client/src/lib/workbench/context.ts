import { getContext, setContext } from 'svelte';

const KEY = Symbol('workbench');

export interface WorkbenchContext {
    /** Slug del componente que se está viendo. */
    readonly component: string;
    /** Slug de la historia aislada (`?story=`), o `null` si se ven todas. */
    readonly isolated: string | null;
}

export function setWorkbench(context: WorkbenchContext): void {
    setContext(KEY, context);
}

export function getWorkbench(): WorkbenchContext {
    return getContext<WorkbenchContext | undefined>(KEY) ?? { component: 'story', isolated: null };
}
