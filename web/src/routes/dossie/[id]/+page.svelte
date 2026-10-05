<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import Dossie from '$lib/components/Dossie.svelte';
	import type { DossiePolitico } from '$lib/types';

	let id = $page.params.id;
	let dossie: DossiePolitico | null = null;
	let loading = true;
	let erro: string | null = null;

	async function carregarDossie() {
		loading = true;
		erro = null;
		try {
			const res = await fetch(`/api/v1/politico/${id}`);
			if (res.ok) {
				dossie = await res.json();
			} else if (res.status === 404) {
				erro = 'Político não encontrado na base de dados.';
			} else {
				erro = 'Falha ao carregar dossiê consolidado do político.';
			}
		} catch (err) {
			erro = 'Erro de comunicação com a API do Radar Cívico.';
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		carregarDossie();
	});
</script>

<div>
	<div class="mb-6">
		<a href="/" class="text-xs font-medium text-slate-400 hover:text-emerald-400 flex items-center gap-1">
			← Voltar à Busca
		</a>
	</div>

	{#if loading}
		<div class="py-20 flex flex-col justify-center items-center gap-3">
			<div class="w-10 h-10 border-3 border-emerald-500 border-t-transparent rounded-full animate-spin"></div>
			<p class="text-sm text-slate-400">Consultando registros do TSE e histórico patrimonial...</p>
		</div>
	{:else if erro}
		<div class="p-8 text-center bg-rose-500/10 border border-rose-500/30 rounded-xl text-rose-300 text-sm">
			{erro}
		</div>
	{:else if dossie}
		<Dossie {dossie} />
	{/if}
</div>
