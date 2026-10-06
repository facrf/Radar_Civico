<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import DossieCpfView from '$lib/components/DossieCpfView.svelte';
	import type { DossieCpf } from '$lib/types';

	let cpfParam = $page.params.cpf;
	let dossie: DossieCpf | null = null;
	let loading = true;
	let erro: string | null = null;

	async function carregarDossie() {
		loading = true;
		erro = null;
		try {
			const res = await fetch(`/api/v1/dossie/cpf/${encodeURIComponent(cpfParam)}`);
			if (res.ok) {
				dossie = await res.json();
			} else if (res.status === 404) {
				erro = 'Pessoa Física não encontrada ou sem vínculos cruzados cadastrados.';
			} else {
				erro = 'Falha ao carregar dossiê analítico da pessoa física.';
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

<svelte:head>
	<title>{dossie ? `${dossie.nome} | Radar Cívico` : 'Dossiê Pessoa Física | Radar Cívico'}</title>
</svelte:head>

<div>
	<div class="mb-6 flex items-center justify-between">
		<a href="/" class="text-xs font-medium text-slate-400 hover:text-emerald-400 flex items-center gap-1.5 transition-colors">
			<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
			</svg>
			<span>Voltar à Busca</span>
		</a>
	</div>

	{#if loading}
		<div class="py-24 flex flex-col justify-center items-center gap-3">
			<div class="w-10 h-10 border-3 border-purple-500 border-t-transparent rounded-full animate-spin"></div>
			<p class="text-sm text-slate-300 font-medium">Cruzando registros de CPF no QSA, TSE, CEAP e OAB...</p>
			<p class="text-xs text-slate-500">Buscando participações societárias e histórico de doações eleitorais</p>
		</div>
	{:else if erro}
		<div class="p-8 text-center bg-rose-500/10 border border-rose-500/30 rounded-2xl text-rose-300 text-sm max-w-xl mx-auto space-y-3">
			<p class="font-semibold">{erro}</p>
			<a href="/" class="inline-block px-4 py-2 bg-slate-800 hover:bg-slate-700 text-xs text-white rounded-lg transition-colors">
				Voltar para pesquisa principal
			</a>
		</div>
	{:else if dossie}
		<DossieCpfView {dossie} />
	{/if}
</div>
