import { invoke } from '@tauri-apps/api/core';

import { APP_VERSION } from '$lib/app';
import { toast } from '$lib/toast';

import { t } from '$lib/i18n';

interface LatestRelease {
    tagName: string;
    htmlUrl: string;
    prerelease: boolean;
    draft: boolean;
}

function parseVersion(version: string): [number, number, number] | undefined {
    const match = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(version);
    if (!match) return undefined;
    return [Number(match[1]), Number(match[2]), Number(match[3])];
}

function isNewer(latest: string, current: string): boolean {
    const newer = parseVersion(latest);
    const installed = parseVersion(current);
    if (!newer || !installed) return false;
    return newer.some((part, index) => part !== installed[index] && part > installed[index]);
}

export async function checkForUpdate(): Promise<void> {
    try {
        const release = await invoke<LatestRelease>('github_latest_release');
        if (
            release.draft ||
            release.prerelease ||
            !release.htmlUrl.startsWith('https://github.com/HormigaDev/blacksmith/releases/')
        )
            return;
        if (isNewer(release.tagName, APP_VERSION)) {
            toast.info(t('updates.available', { version: release.tagName }), {
                description: t('updates.description', { url: release.htmlUrl }),
                duration: 0,
            });
        }
    } catch (error) {
        // La comprobación es opcional: no debe impedir el arranque sin conexión.
        console.warn('No se pudo comprobar si hay una actualización disponible', error);
    }
}
