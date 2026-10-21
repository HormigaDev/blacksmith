/**
 * Notificaciones efímeras. Los textos llegan ya traducidos: `toast.success(t('…'))`.
 * Para errores capturados, `toast.error(describeError(error))` de `$lib/http`.
 */

export type ToastVariant = 'info' | 'success' | 'warning' | 'danger';

export interface ToastOptions {
    description?: string;
    /** Milisegundos hasta cerrarse sola; 0 la deja fija. */
    duration?: number;
}

export interface Toast {
    readonly id: number;
    readonly variant: ToastVariant;
    readonly title: string;
    readonly description: string | undefined;
}

/** Los errores duran más: hay que poder leerlos. */
const DEFAULT_DURATION: Record<ToastVariant, number> = { info: 5000, success: 4000, warning: 7000, danger: 9000 };
const MAX_VISIBLE = 5;

class ToastState {
    #items = $state<Toast[]>([]);
    #next = 0;
    #timers = new Map<number, ReturnType<typeof setTimeout>>();

    get items(): readonly Toast[] {
        return this.#items;
    }

    show(variant: ToastVariant, title: string, options: ToastOptions = {}): number {
        const id = ++this.#next;
        this.#items = [...this.#items, { id, variant, title, description: options.description }];
        while (this.#items.length > MAX_VISIBLE) {
            const oldest = this.#items[0];
            if (oldest) this.dismiss(oldest.id);
        }
        const duration = options.duration ?? DEFAULT_DURATION[variant];
        if (duration > 0)
            this.#timers.set(
                id,
                setTimeout(() => this.dismiss(id), duration),
            );
        return id;
    }

    info = (title: string, options?: ToastOptions) => this.show('info', title, options);
    success = (title: string, options?: ToastOptions) => this.show('success', title, options);
    warning = (title: string, options?: ToastOptions) => this.show('warning', title, options);
    error = (title: string, options?: ToastOptions) => this.show('danger', title, options);

    dismiss(id: number): void {
        clearTimeout(this.#timers.get(id));
        this.#timers.delete(id);
        this.#items = this.#items.filter((item) => item.id !== id);
    }
}

export const toast = new ToastState();
