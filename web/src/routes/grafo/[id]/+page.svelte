<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import GrafoRede from '$lib/components/GrafoRede.svelte';
	import type { SubgrafoData } from '$lib/types';

	let id = $page.params.id;
	let grau = 2;
	let subgrafo: SubgrafoData | null = null;
	let loading = true;
	let erro: string | null = null;

	async function carregarGrafo() {
		loading = true;
		erro = null;
		try {
			const res = await fetch(`/api/v1/grafo/${id}?grau=${grau}`);
			if (res.ok) {
				subgrafo = await res.json();
			} else if (res.status === 404) {
				erro = 'Nó ou entidade não localizada no grafo relacional.';
			} else {
				erro = 'Falha ao processar extração de subgrafo.';
			}
		} catch (err) {
			erro = 'Erro ao conectar ao serviço de grafos.';
		} finally {
			loading = false;
		}
	}

	function alterarGrau(novoGrau: number) {
		grau = novoGrau;
		carregarGrafo();
	}

	onMount(() => {
		carregarGrafo();
	});
</script>

<div class="space-y-6">
	<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
		<div>
			<a href="/" class="text-xs font-medium text-slate-400 hover:text-emerald-400 flex items-center gap-1 mb-2">
				← Voltar ao Início
			</a>
			<h1 class="text-2xl sm:text-3xl font-bold text-white tracking-tight">Grafo de Relacionamentos</h1>
			<p class="text-sm text-slate-400 mt-0.5">Visualização interativa de conexões, fluxos financeiros e detecção de nós centrais.</p>
		</div>

		<div class="flex items-center gap-2 bg-slate-800/80 p-1.5 rounded-xl border border-slate-700/80">
			<span class="text-xs text-slate-400 px-2 font-medium">Profundidade:</span>
			<button
				on:click={() => alterarGrau(1)}
				class="px-2.5 py-1 text-xs font-bold rounded-lg transition-colors"
				class:bg-emerald-600={grau === 1}
				class:text-white={grau === 1}
				class:bg-slate-700={grau !== 1}
				class:text-slate-300={grau !== 1}
			>
				1º Grau
			</button>
			<button
				on:click={() => alterarGrau(2)}
				class="px-2.5 py-1 text-xs font-bold rounded-lg transition-colors"
				class:bg-emerald-600={grau === 2}
				class:text-white={grau === 2}
				class:bg-slate-700={grau !== 2}
				class:text-slate-300={grau !== 2}
			>
				2º Grau
			</button>
			<button
				on:click={() => alterarGrau(3)}
				class="px-2.5 py-1 text-xs font-bold rounded-lg transition-colors"
				class:bg-emerald-600={grau === 3}
				class:text-white={grau === 3}
				class:bg-slate-700={grau !== 3}
				class:text-slate-300={grau !== 3}
			>
				3º Grau
			</button>
		</div>
	</div>

	{#if loading}
		<div class="py-24 flex flex-col justify-center items-center gap-3 bg-slate-950 border border-slate-800 rounded-2xl">
			<div class="w-10 h-10 border-3 border-emerald-500 border-t-transparent rounded-full animate-spin"></div>
			<p class="text-sm text-slate-400">Extraindo conexões e calculando topologia de rede...</p>
		</div>
	{:else if erro}
		<div class="p-8 text-center bg-rose-500/10 border border-rose-500/30 rounded-xl text-rose-300 text-sm">
			{erro}
		</div>
	{:else if subgrafo}
		<GrafoRede dados={subgrafo} />
	{/if}
</div>
