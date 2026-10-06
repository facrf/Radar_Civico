import { writable } from 'svelte/store';

export interface VersionInfo {
	version: string;
	commit: string;
	count: number;
}

// Valores injetados no momento do build pelo Vite / Git
const defaultVersion = typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : 'v0.001';
const defaultCommit = typeof __APP_COMMIT__ !== 'undefined' ? __APP_COMMIT__ : 'unknown';
const defaultCount = typeof __APP_COUNT__ !== 'undefined' ? __APP_COUNT__ : 1;

export const versionStore = writable<VersionInfo>({
	version: defaultVersion,
	commit: defaultCommit,
	count: defaultCount
});

/**
 * Consulta o backend para sincronizar a versão do servidor em tempo de execução
 */
export async function sincronizarVersaoServidor(): Promise<VersionInfo> {
	try {
		const res = await fetch('/api/v1/config/versao');
		if (res.ok) {
			const data = await res.json();
			const info: VersionInfo = {
				version: data.versao || defaultVersion,
				commit: data.commit || defaultCommit,
				count: data.count || defaultCount
			};
			versionStore.set(info);
			return info;
		}
	} catch (err) {
		// Mantém o valor estático pré-injetado em caso de falha de rede
	}
	return {
		version: defaultVersion,
		commit: defaultCommit,
		count: defaultCount
	};
}
