/**
 * Universal load — runs both during SSR and on client-side navigation.
 *
 * We don't actually hit the API here: the detail view triggers its own
 * fetch in `onMount`. The only thing this load function does is pass
 * the route param down as `data.id` so the Svelte template can use it
 * without re-parsing `$page.params` everywhere.
 */
import type { PageLoad } from './$types.js';

export const load: PageLoad = ({ params }) => {
  return { id: params.id };
};
