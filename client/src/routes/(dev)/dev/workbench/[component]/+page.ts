import { error } from '@sveltejs/kit';

import { findStory } from '$lib/workbench';

import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => {
    const story = findStory(params.component);
    if (!story) error(404, `No hay historias para "${params.component}"`);
    const module = await story.load();
    return { slug: story.slug, meta: module.meta, component: module.default };
};
