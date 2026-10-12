import { page as state } from '$app/state';
import { toStore } from 'svelte/store';
// Snapshot dos campos: preserva reatividade nas telas legadas durante navegação.
export const page = toStore(() => ({
    data: state.data, error: state.error, form: state.form, params: { ...state.params },
    route: state.route, status: state.status, url: state.url,
}));
