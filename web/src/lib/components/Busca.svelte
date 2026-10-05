<script lang="ts">
	import type { ItemBusca } from '$lib/types';

	let query = '';
	let loading = false;
	let resultados: ItemBusca[] = [];
	let debounceTimer: ReturnType<typeof setTimeout>;

	async function executarBusca() {
		if (!query.trim()) {
			resultados = [];
			return;
		}

		loading = true;
		try {
			const res = await fetch(`/api/v1/busca?q=${encodeURIComponent(query.trim())}`);
			if (res.ok) {
				const data = await res.json();
				resultados = data.itens || [];
			} else {
				resultados = [];
			}
		} catch (err) {
			console.error('Erro na busca:', err);
			resultados = [];
		} finally {
			loading = false;
		}
	}

	function handleInput() {
		clearTimeout(debounceTimer);
		debounceTimer = setTimeout(() => {
			executarBusca();
		}, 300);
	}
</script>

<div class="w-full max-w-3xl mx-auto">
	<div class="relative">
		<div class="absolute inset-y-0 left-0 pl-4 flex items-center pointer-events-none text-slate-400">
			<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
			</svg>
		</div>

		<input
			type="text"
			bind:value={query}
			on:input={handleInput}
			placeholder="Buscar político, doador ou fornecedor por nome, CPF ou CNPJ..."
			class="w-full pl-11 pr-12 py-3.5 bg-slate-800/90 border border-slate-700 rounded-xl text-slate-100 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:border-transparent text-sm sm:text-base shadow-lg transition-all"
		/>

		{#if loading}
			<div class="absolute inset-y-0 right-0 pr-4 flex items-center">
				<div class="w-5 h-5 border-2 border-emerald-500 border-t-transparent rounded-full animate-spin"></div>
			</div>
		{/if}
	</div>

	{#if resultados.length > 0}
		<div class="mt-4 bg-slate-800/90 border border-slate-700/80 rounded-xl overflow-hidden shadow-2xl divide-y divide-slate-700/50">
			{#each resultados as item}
				<div class="p-4 hover:bg-slate-750 transition-colors flex items-center justify-between gap-4">
					<div class="flex items-center gap-3">
						<span
							class="px-2.5 py-1 text-xs font-semibold rounded-full tracking-wide"
							class:bg-blue-500-20={item.tipo === 'POLITICO'}
							class:text-blue-400={item.tipo === 'POLITICO'}
							class:bg-emerald-500-20={item.tipo === 'DOADOR'}
							class:text-emerald-400={item.tipo === 'DOADOR'}
							class:bg-amber-500-20={item.tipo === 'FORNECEDOR'}
							class:text-amber-400={item.tipo === 'FORNECEDOR'}
							style="background-color: rgba(30, 41, 59, 0.8);"
						>
							{item.tipo}
						</span>
						<div>
							<h4 class="font-medium text-slate-100 text-sm sm:text-base">{item.nome}</h4>
							<p class="text-xs text-slate-400 mt-0.5">
								{item.identificador}
								{#if item.detalhe}
									• <span class="text-slate-300">{item.detalhe}</span>
								{/if}
							</p>
						</div>
					</div>

					<div class="flex items-center gap-2">
						{#if item.tipo === 'POLITICO'}
							<a
								href={`/dossie/${item.id}`}
								class="px-3 py-1.5 text-xs font-medium bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg transition-colors shadow"
							>
								Ver Dossiê
							</a>
						{/if}
						<a
							href={`/grafo/${item.id}?grau=2`}
							class="px-3 py-1.5 text-xs font-medium bg-slate-700 hover:bg-slate-600 text-slate-200 rounded-lg transition-colors"
						>
							Ver Grafo
						</a>
					</div>
				</div>
			{/each}
		</div>
	{:else if query.trim().length > 1 && !loading}
		<div class="mt-4 p-6 text-center text-sm text-slate-400 bg-slate-800/40 rounded-xl border border-slate-800">
			Nenhum registro encontrado para "{query}".
		</div>
	{/if}
</div>
