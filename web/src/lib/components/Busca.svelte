<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { ItemBusca } from '$lib/types';

	let query = '';
	let request: AbortController | null = null;
	let loading = false;
	let resultados: ItemBusca[] = [];
	let buscou = false;
	let inputRef: HTMLInputElement;

	async function executarBusca() {
		request?.abort();
		const current = new AbortController();
		request = current;
		const termo = query.trim();
		if (!termo) {
			resultados = [];
			buscou = false;
			return;
		}

		loading = true;
		buscou = true;
		try {
			const res = await fetch(`/api/v1/busca?q=${encodeURIComponent(termo)}`, { signal: current.signal });
			if (res.ok) {
				const data = await res.json();
				if (request !== current) return;
				resultados = data.resultados || data.itens || [];
			} else {
				resultados = [];
			}
		} catch (err) {
			if (!current.signal.aborted) { console.error('Erro na busca:', err); resultados = []; }
		} finally {
			if (request === current) loading = false;
		}
	}

	function limparBusca() {
		request?.abort();
		request = null;
		loading = false;
		query = '';
		resultados = [];
		buscou = false;
		if (inputRef) {
			inputRef.focus();
		}
	}

	function corTipo(tipo: string): { bg: string; text: string; border: string; label: string } {
		const t = tipo.toUpperCase();
		if (t === 'POLITICO') {
			return { bg: 'bg-blue-500/10', text: 'text-blue-400', border: 'border-blue-500/30', label: 'Político' };
		}
		if (t === 'DOADOR') {
			return { bg: 'bg-emerald-500/10', text: 'text-emerald-400', border: 'border-emerald-500/30', label: 'Doador' };
		}
		if (t === 'FORNECEDOR') {
			return { bg: 'bg-amber-500/10', text: 'text-amber-400', border: 'border-amber-500/30', label: 'Fornecedor' };
		}
		if (t === 'SOCIO' || t === 'SOCIO_QSA') {
			return { bg: 'bg-purple-500/10', text: 'text-purple-400', border: 'border-purple-500/30', label: 'Sócio (QSA)' };
		}
		if (t === 'EMPRESA_QSA') {
			return { bg: 'bg-cyan-500/10', text: 'text-cyan-400', border: 'border-cyan-500/30', label: 'Empresa (QSA)' };
		}
		return { bg: 'bg-slate-700/30', text: 'text-slate-300', border: 'border-slate-700', label: tipo };
	}

	function obterUrlDossie(item: ItemBusca): string {
		const t = (item.tipo || '').toUpperCase();
		const doc = item.documento || item.identificador || '';
		const digitos = doc.replace(/\D/g, '');

		if (t === 'POLITICO' && item.id) {
			return `/dossie/${item.id}`;
		}

		if (t === 'EMPRESA_QSA') {
			if (digitos.length >= 8) {
				return `/dossie/cnpj/${digitos}`;
			}
		}

		if (t === 'SOCIO' || t === 'SOCIO_QSA') {
			if (item.documento && (item.documento.includes('***') || digitos.length >= 6)) {
				return `/dossie/cpf/${encodeURIComponent(item.documento)}`;
			}
			if (item.nome || item.titulo) {
				return `/dossie/cpf/${encodeURIComponent(item.nome || item.titulo || '')}`;
			}
		}

		if (t === 'FORNECEDOR' || t === 'DOADOR') {
			if (digitos.length === 14 || digitos.length === 8) {
				return `/dossie/cnpj/${digitos}`;
			}
			if (digitos.length === 11 || doc.includes('***')) {
				return `/dossie/cpf/${encodeURIComponent(doc)}`;
			}
		}

		// Fallbacks por formato de dígitos
		if (digitos.length === 14 || digitos.length === 8) {
			return `/dossie/cnpj/${digitos}`;
		}
		if (digitos.length === 11 || doc.includes('***')) {
			return `/dossie/cpf/${encodeURIComponent(doc)}`;
		}
		if (item.id) {
			return `/dossie/${item.id}`;
		}
		if (item.nome || item.titulo) {
			return `/dossie/cpf/${encodeURIComponent(item.nome || item.titulo || '')}`;
		}

		return '#';
	}

	function obterUrlGrafo(item: ItemBusca): string {
		if (item.id) {
			return `/grafo/${item.id}?grau=2`;
		}
		const doc = item.documento || item.identificador || '';
		const digitos = doc.replace(/\D/g, '');
		const t = (item.tipo || '').toUpperCase();

		if (t === 'EMPRESA_QSA' || digitos.length === 14 || digitos.length === 8) {
			return `/grafo/cnpj_${digitos}?grau=2`;
		}
		if (t === 'SOCIO' || t === 'SOCIO_QSA') {
			return `/grafo/socio_${encodeURIComponent(item.nome || item.titulo || doc)}?grau=2`;
		}
		return `/grafo/${encodeURIComponent(doc || item.titulo || '')}?grau=2`;
	}
	onDestroy(() => request?.abort());
</script>

<div class="w-full max-w-3xl mx-auto">
	<form on:submit|preventDefault={executarBusca} class="relative flex flex-col sm:flex-row items-stretch sm:items-center gap-2">
		<div class="relative flex-1">
			<div class="absolute inset-y-0 left-0 pl-4 flex items-center pointer-events-none text-slate-400">
				<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
				</svg>
			</div>

			<input
				bind:this={inputRef}
				type="text"
				bind:value={query}
				placeholder="Buscar político, doador, fornecedor ou sócio por nome, CPF ou CNPJ..."
				class="w-full pl-11 pr-10 py-3.5 bg-slate-800/90 border border-slate-700 rounded-xl text-slate-100 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:border-transparent text-sm sm:text-base shadow-lg transition-all"
			/>

			{#if query.trim().length > 0}
				<button
					type="button"
					on:click={limparBusca}
					class="absolute inset-y-0 right-0 pr-3.5 flex items-center text-slate-400 hover:text-slate-200 transition-colors"
					title="Limpar busca"
				>
					<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
					</svg>
				</button>
			{/if}
		</div>

		<button
			type="submit"
			disabled={loading || !query.trim()}
			class="px-6 py-3.5 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 disabled:cursor-not-allowed text-white font-semibold rounded-xl text-sm sm:text-base transition-all shadow-lg flex items-center justify-center gap-2 flex-shrink-0"
		>
			{#if loading}
				<div class="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
				<span>Buscando...</span>
			{:else}
				<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
				</svg>
				<span>Buscar</span>
			{/if}
		</button>
	</form>

	<!-- Resultados da Busca -->
	{#if resultados.length > 0}
		<div class="mt-4 bg-slate-800/90 border border-slate-700/80 rounded-xl overflow-hidden shadow-2xl divide-y divide-slate-700/50">
			<div class="px-4 py-2.5 bg-slate-900/60 border-b border-slate-700/50 flex items-center justify-between text-xs text-slate-400">
				<span>Resultados encontrados: <strong class="text-slate-200">{resultados.length}</strong></span>
				<span>Clique no card para abrir o dossiê analítico</span>
			</div>
			{#each resultados as item}
				{@const badge = corTipo(item.tipo)}
				{@const urlDossie = obterUrlDossie(item)}
				{@const urlGrafo = obterUrlGrafo(item)}
				<a
					href={urlDossie}
					class="p-4 hover:bg-slate-750/90 transition-all flex flex-col sm:flex-row sm:items-center justify-between gap-4 group cursor-pointer border-l-4 border-transparent hover:border-emerald-500 block no-underline"
				>
					<div class="flex items-start sm:items-center gap-3">
						<span class={`px-2.5 py-1 text-xs font-semibold rounded-full tracking-wide border ${badge.bg} ${badge.text} ${badge.border} flex-shrink-0 mt-0.5 sm:mt-0`}>
							{badge.label}
						</span>
						<div>
							<h4 class="font-medium text-slate-100 text-sm sm:text-base leading-snug group-hover:text-emerald-400 transition-colors flex items-center gap-1.5">
								<span>{item.titulo || item.nome}</span>
								<svg class="w-4 h-4 text-slate-500 group-hover:text-emerald-400 group-hover:translate-x-0.5 transition-all opacity-0 group-hover:opacity-100" fill="none" viewBox="0 0 24 24" stroke="currentColor">
									<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
								</svg>
							</h4>
							<p class="text-xs text-slate-400 mt-0.5 leading-relaxed">
								<span class="font-mono text-slate-300">{item.documento || item.identificador}</span>
								{#if item.subtitulo || item.detalhe}
									• <span class="text-slate-300">{item.subtitulo || item.detalhe}</span>
								{/if}
							</p>
						</div>
					</div>

					<div class="flex items-center gap-2 self-end sm:self-auto flex-shrink-0">
						<span
							class="px-3 py-1.5 text-xs font-medium bg-emerald-600 group-hover:bg-emerald-500 text-white rounded-lg transition-colors shadow flex items-center gap-1.5"
						>
							<span>Ver Dossiê</span>
							<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 5l7 7m0 0l-7 7m7-7H3" />
							</svg>
						</span>
						<a
							href={urlGrafo}
							on:click|stopPropagation
							class="px-3 py-1.5 text-xs font-medium bg-slate-700 hover:bg-slate-600 text-slate-200 rounded-lg transition-colors flex items-center gap-1"
							title="Visualizar conexões e rede no grafo"
						>
							<svg class="w-3.5 h-3.5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
							</svg>
							<span>Grafo</span>
						</a>
					</div>
				</a>
			{/each}
		</div>
	{:else if buscou && !loading}
		<div class="mt-4 p-6 text-center text-sm text-slate-400 bg-slate-800/40 rounded-xl border border-slate-800 space-y-1">
			<p class="text-slate-300 font-medium">Nenhum registro encontrado para "{query}".</p>
			<p class="text-xs text-slate-500">
				Dica: Para pesquisar por CPF, informe os 11 dígitos (ex: 123.456.789-00 ou 12345678900) ou a máscara QSA da Receita. Para empresas, informe o CNPJ.
			</p>
		</div>
	{/if}
</div>
