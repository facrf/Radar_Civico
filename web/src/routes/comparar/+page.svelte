<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '#lib/page';
	import { goto } from '$app/navigation';
	import AvaliacaoIntegridade from '#lib/components/AvaliacaoIntegridade.svelte';
	import type { PoliticoDetalheResponse, ItemPoliticoListagem } from '#lib/types';

	let id1 = $page.url.searchParams.get('p1') || '';
	let id2 = $page.url.searchParams.get('p2') || '';

	let politico1: PoliticoDetalheResponse | null = null;
	let politico2: PoliticoDetalheResponse | null = null;

	let loading1 = false;
	let loading2 = false;
	let erro1: string | null = null;
	let erro2: string | null = null;

	// Busca e Autocomplete
	let busca1 = '';
	let busca2 = '';
	let sugestoes1: ItemPoliticoListagem[] = [];
	let sugestoes2: ItemPoliticoListagem[] = [];
	let buscando1 = false;
	let buscando2 = false;

	// Sugestões populares para início rápido
	let politicosPopulares: ItemPoliticoListagem[] = [];

	function formatarMoeda(val: number): string {
		return new Intl.NumberFormat('pt-BR', { style: 'currency', currency: 'BRL' }).format(val);
	}

	function getPartidoColor(partido: string): string {
		const p = (partido || '').toUpperCase();
		if (p.includes('PT')) return 'from-red-600 to-rose-700';
		if (p.includes('PL')) return 'from-blue-600 to-indigo-700';
		if (p.includes('UNIÃO') || p.includes('UNIAO')) return 'from-sky-600 to-blue-700';
		if (p.includes('PP')) return 'from-blue-700 to-cyan-800';
		if (p.includes('MDB')) return 'from-emerald-700 to-green-800';
		if (p.includes('PSD')) return 'from-amber-600 to-orange-700';
		if (p.includes('REPUBLICANOS')) return 'from-blue-800 to-slate-800';
		if (p.includes('PSOL')) return 'from-amber-500 to-yellow-600';
		return 'from-slate-700 to-slate-900';
	}

	function getIniciais(nome: string): string {
		if (!nome) return '??';
		const partes = nome.trim().split(/\s+/);
		if (partes.length === 1) return partes[0].slice(0, 2).toUpperCase();
		return (partes[0][0] + partes[partes.length - 1][0]).toUpperCase();
	}

	function totalAlertas(p: PoliticoDetalheResponse | null): number {
		if (!p) return 0;
		return (
			(p.alertas_parentesco?.length || 0) +
			(p.alertas_auxilio?.length || 0) +
			(p.alertas_evolucao_patrimonial?.length || 0)
		);
	}

	function calcEmendasEmpenhado(p: PoliticoDetalheResponse | null): number {
		if (!p?.emendas) return 0;
		return p.emendas.reduce((acc, e) => acc + (e.valor_empenhado || 0), 0);
	}

	function calcEmendasPago(p: PoliticoDetalheResponse | null): number {
		if (!p?.emendas) return 0;
		return p.emendas.reduce((acc, e) => acc + (e.valor_pago || 0), 0);
	}

	async function carregarPolitico(id: string, slot: 1 | 2) {
		if (!id) {
			if (slot === 1) politico1 = null;
			else politico2 = null;
			return;
		}

		if (slot === 1) {
			loading1 = true;
			erro1 = null;
		} else {
			loading2 = true;
			erro2 = null;
		}

		try {
			const res = await fetch(`/api/politicos/${id}`);
			if (!res.ok) {
				const msg = res.status === 404 ? 'Parlamentar não encontrado.' : 'Erro ao carregar dados.';
				if (slot === 1) erro1 = msg;
				else erro2 = msg;
				return;
			}
			const data: PoliticoDetalheResponse = await res.json();
			if (slot === 1) politico1 = data;
			else politico2 = data;
		} catch (e: any) {
			const msg = 'Falha na comunicação com a API.';
			if (slot === 1) erro1 = msg;
			else erro2 = msg;
		} finally {
			if (slot === 1) loading1 = false;
			else loading2 = false;
		}
	}

	let debounceTimer1: any;
	function onBusca1Input() {
		clearTimeout(debounceTimer1);
		if (busca1.trim().length < 2) {
			sugestoes1 = [];
			return;
		}
		debounceTimer1 = setTimeout(async () => {
			buscando1 = true;
			try {
				const res = await fetch(`/api/politicos?q=${encodeURIComponent(busca1)}&limit=6`);
				if (res.ok) {
					const data = await res.json();
					sugestoes1 = data.politicos || [];
				}
			} finally {
				buscando1 = false;
			}
		}, 300);
	}

	let debounceTimer2: any;
	function onBusca2Input() {
		clearTimeout(debounceTimer2);
		if (busca2.trim().length < 2) {
			sugestoes2 = [];
			return;
		}
		debounceTimer2 = setTimeout(async () => {
			buscando2 = true;
			try {
				const res = await fetch(`/api/politicos?q=${encodeURIComponent(busca2)}&limit=6`);
				if (res.ok) {
					const data = await res.json();
					sugestoes2 = data.politicos || [];
				}
			} finally {
				buscando2 = false;
			}
		}, 300);
	}

	function selecionarPolitico(p: ItemPoliticoListagem, slot: 1 | 2) {
		if (slot === 1) {
			id1 = `${p.id}`;
			busca1 = '';
			sugestoes1 = [];
			carregarPolitico(id1, 1);
		} else {
			id2 = `${p.id}`;
			busca2 = '';
			sugestoes2 = [];
			carregarPolitico(id2, 2);
		}
		atualizarUrl();
	}

	function trocarPoliticos() {
		const tempId = id1;
		const tempPolitico = politico1;
		id1 = id2;
		politico1 = politico2;
		id2 = tempId;
		politico2 = tempPolitico;
		atualizarUrl();
	}

	function removerPolitico(slot: 1 | 2) {
		if (slot === 1) {
			id1 = '';
			politico1 = null;
		} else {
			id2 = '';
			politico2 = null;
		}
		atualizarUrl();
	}

	function atualizarUrl() {
		const params = new URLSearchParams();
		if (id1) params.set('p1', id1);
		if (id2) params.set('p2', id2);
		const query = params.toString();
		goto(query ? `/comparar?${query}` : '/comparar', { replaceState: true });
	}

	async function carregarPopulares() {
		try {
			const res = await fetch('/api/politicos?limit=8&apenas_com_gastos=true');
			if (res.ok) {
				const data = await res.json();
				politicosPopulares = data.politicos || [];
			}
		} catch (e) {
			console.warn('Erro ao carregar populares:', e);
		}
	}

	onMount(() => {
		carregarPopulares();
		if (id1) carregarPolitico(id1, 1);
		if (id2) carregarPolitico(id2, 2);
	});
</script>

<svelte:head>
	<title>Comparador de Parlamentares • Radar Cívico</title>
</svelte:head>

<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 space-y-8">
	<!-- Cabeçalho Institucional de Emissão Oficial (Exibido Apenas na Impressão / PDF) -->
	<div class="hidden print:block border-b-2 border-slate-900 pb-4 mb-4">
		<div class="flex items-center justify-between">
			<div class="flex items-center gap-3">
				<div class="w-10 h-10 rounded-xl bg-slate-900 text-white flex items-center justify-center font-black text-xl">
					RC
				</div>
				<div>
					<h1 class="text-xl font-black text-slate-900 tracking-tight">RADAR CÍVICO • RELATÓRIO COMPARATIVO</h1>
					<p class="text-[10px] text-slate-600 font-mono tracking-wider uppercase">Auditoria Cívica Pareada • Fontes Públicas Oficiais (TSE / CEAP / SIOP)</p>
				</div>
			</div>
			<div class="text-right text-[10px] text-slate-700 font-mono space-y-0.5">
				<div>Emissão: {new Date().toLocaleDateString('pt-BR')} às {new Date().toLocaleTimeString('pt-BR')}</div>
				<div>Chave Verificadora SHA-256</div>
			</div>
		</div>
	</div>

	<!-- Topo da Página -->
	<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4 print:hidden">
		<div>
			<a href="/politicos" class="text-xs font-semibold text-slate-400 hover:text-emerald-400 flex items-center gap-1.5 transition-colors mb-2">
				← Voltar ao Catálogo
			</a>
			<h1 class="text-2xl sm:text-3xl font-black text-white tracking-tight flex items-center gap-2.5">
				<span>⚖️ Comparador de Parlamentares</span>
			</h1>
			<p class="text-xs sm:text-sm text-slate-400 mt-1">
				Analise lado a lado os gastos da CEAP, emendas orçamentárias, patrimônio declarado e scores de integridade de dois políticos.
			</p>
		</div>

		<div class="flex items-center gap-2">
			{#if politico1 && politico2}
				<button
					type="button"
					on:click={trocarPoliticos}
					class="px-3 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-200 text-xs font-semibold rounded-xl transition-colors flex items-center gap-1.5 shadow-sm"
					title="Inverter ordem dos parlamentares selecionados"
				>
					<span>⇄ Inverter</span>
				</button>
			{/if}

			<button
				type="button"
				on:click={() => window.print()}
				class="px-3 py-2 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-xl transition-colors flex items-center gap-1.5 shadow-md"
				title="Imprimir ou salvar relatório comparativo em PDF"
			>
				<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4a2 2 0 00-2-2H9a2 2 0 00-2 2v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
				</svg>
				<span>Exportar Relatório PDF</span>
			</button>
		</div>
	</div>

	<!-- Seletores de Parlamentares (Slot 1 vs Slot 2) -->
	<div class="grid grid-cols-1 md:grid-cols-2 gap-6 print:hidden">
		<!-- Slot 1 -->
		<div class="bg-slate-900 border border-slate-800 rounded-3xl p-5 shadow-xl space-y-4 relative">
			<div class="flex items-center justify-between pb-3 border-b border-slate-800">
				<span class="text-xs font-bold uppercase tracking-wider text-emerald-400 font-mono">Parlamentar A (Referência)</span>
				{#if politico1}
					<button
						type="button"
						on:click={() => removerPolitico(1)}
						class="text-xs text-rose-400 hover:text-rose-300 font-semibold"
					>
						Substituir ✕
					</button>
				{/if}
			</div>

			{#if politico1}
				<div class="flex items-center gap-4">
					{#if politico1.foto_base64}
						<img
							src={`data:${politico1.foto_mime || 'image/jpeg'};base64,${politico1.foto_base64}`}
							alt={politico1.nome_urna}
							class="w-16 h-16 rounded-2xl object-cover border-2 border-emerald-500/50 shadow-md bg-slate-950"
						/>
					{:else}
						<div class="w-16 h-16 rounded-2xl bg-gradient-to-br {getPartidoColor(politico1.partido)} flex items-center justify-center font-extrabold text-xl text-white shadow-md">
							{getIniciais(politico1.nome_urna)}
						</div>
					{/if}
					<div>
						<h2 class="text-lg font-black text-white">{politico1.nome_urna}</h2>
						<p class="text-xs text-slate-300">{politico1.nome_completo}</p>
						<div class="flex items-center gap-2 mt-1 text-xs font-mono">
							<span class="px-2 py-0.5 rounded bg-slate-800 text-emerald-300 font-bold">{politico1.partido}</span>
							<span class="px-2 py-0.5 rounded bg-slate-800 text-slate-300 font-bold">{politico1.uf}</span>
							<span class="text-slate-400 font-sans">{politico1.cargo}</span>
						</div>
					</div>
				</div>
			{:else}
				<div class="space-y-3">
					<p class="text-xs text-slate-400">Pesquise o primeiro parlamentar por nome:</p>
					<div class="relative">
						<input
							type="text"
							bind:value={busca1}
							on:input={onBusca1Input}
							placeholder="Ex: Arthur Lira, Tabata Amaral, Nikolas..."
							class="w-full bg-slate-950 border border-slate-700 rounded-xl px-4 py-2.5 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-emerald-400"
						/>
						{#if buscando1}
							<div class="absolute right-3 top-3 w-4 h-4 border-2 border-emerald-400 border-t-transparent rounded-full animate-spin"></div>
						{/if}
					</div>

					{#if sugestoes1.length > 0}
						<div class="bg-slate-950 border border-slate-800 rounded-2xl p-2 space-y-1 max-h-56 overflow-y-auto shadow-2xl">
							{#each sugestoes1 as s}
								<button
									type="button"
									on:click={() => selecionarPolitico(s, 1)}
									class="w-full text-left p-2 hover:bg-slate-850 rounded-xl flex items-center justify-between text-xs transition-colors"
								>
									<div>
										<strong class="text-white block">{s.nome_urna}</strong>
										<span class="text-[11px] text-slate-400">{s.sigla_partido} - {s.uf} • {s.cargo}</span>
									</div>
									<span class="text-emerald-400 font-semibold">Selecionar →</span>
								</button>
							{/each}
						</div>
					{/if}
				</div>
			{/if}
		</div>

		<!-- Slot 2 -->
		<div class="bg-slate-900 border border-slate-800 rounded-3xl p-5 shadow-xl space-y-4 relative">
			<div class="flex items-center justify-between pb-3 border-b border-slate-800">
				<span class="text-xs font-bold uppercase tracking-wider text-cyan-400 font-mono">Parlamentar B (Comparação)</span>
				{#if politico2}
					<button
						type="button"
						on:click={() => removerPolitico(2)}
						class="text-xs text-rose-400 hover:text-rose-300 font-semibold"
					>
						Substituir ✕
					</button>
				{/if}
			</div>

			{#if politico2}
				<div class="flex items-center gap-4">
					{#if politico2.foto_base64}
						<img
							src={`data:${politico2.foto_mime || 'image/jpeg'};base64,${politico2.foto_base64}`}
							alt={politico2.nome_urna}
							class="w-16 h-16 rounded-2xl object-cover border-2 border-cyan-500/50 shadow-md bg-slate-950"
						/>
					{:else}
						<div class="w-16 h-16 rounded-2xl bg-gradient-to-br {getPartidoColor(politico2.partido)} flex items-center justify-center font-extrabold text-xl text-white shadow-md">
							{getIniciais(politico2.nome_urna)}
						</div>
					{/if}
					<div>
						<h2 class="text-lg font-black text-white">{politico2.nome_urna}</h2>
						<p class="text-xs text-slate-300">{politico2.nome_completo}</p>
						<div class="flex items-center gap-2 mt-1 text-xs font-mono">
							<span class="px-2 py-0.5 rounded bg-slate-800 text-cyan-300 font-bold">{politico2.partido}</span>
							<span class="px-2 py-0.5 rounded bg-slate-800 text-slate-300 font-bold">{politico2.uf}</span>
							<span class="text-slate-400 font-sans">{politico2.cargo}</span>
						</div>
					</div>
				</div>
			{:else}
				<div class="space-y-3">
					<p class="text-xs text-slate-400">Pesquise o segundo parlamentar por nome:</p>
					<div class="relative">
						<input
							type="text"
							bind:value={busca2}
							on:input={onBusca2Input}
							placeholder="Ex: Hugo Motta, Erika Hilton, Rodrigo Maia..."
							class="w-full bg-slate-950 border border-slate-700 rounded-xl px-4 py-2.5 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-cyan-400"
						/>
						{#if buscando2}
							<div class="absolute right-3 top-3 w-4 h-4 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></div>
						{/if}
					</div>

					{#if sugestoes2.length > 0}
						<div class="bg-slate-950 border border-slate-800 rounded-2xl p-2 space-y-1 max-h-56 overflow-y-auto shadow-2xl">
							{#each sugestoes2 as s}
								<button
									type="button"
									on:click={() => selecionarPolitico(s, 2)}
									class="w-full text-left p-2 hover:bg-slate-850 rounded-xl flex items-center justify-between text-xs transition-colors"
								>
									<div>
										<strong class="text-white block">{s.nome_urna}</strong>
										<span class="text-[11px] text-slate-400">{s.sigla_partido} - {s.uf} • {s.cargo}</span>
									</div>
									<span class="text-cyan-400 font-semibold">Selecionar →</span>
								</button>
							{/each}
						</div>
					{/if}
				</div>
			{/if}
		</div>
	</div>

	<!-- Sugestões Rápidas de Parlamentares para preencher caso nenhum esteja selecionado -->
	{#if (!politico1 || !politico2) && politicosPopulares.length > 0}
		<div class="bg-slate-900/50 border border-slate-800/80 rounded-2xl p-4 print:hidden">
			<span class="text-xs font-bold text-slate-400 uppercase tracking-wider block mb-2.5">
				Sugestões para Comparação Rápida:
			</span>
			<div class="flex flex-wrap gap-2">
				{#each politicosPopulares as pop}
					<button
						type="button"
						on:click={() => {
							if (!politico1) selecionarPolitico(pop, 1);
							else if (!politico2) selecionarPolitico(pop, 2);
						}}
						class="px-2.5 py-1.5 rounded-lg bg-slate-800/80 hover:bg-slate-700 text-xs text-slate-300 hover:text-white border border-slate-700 transition-colors flex items-center gap-1.5"
					>
						<span class="font-bold">{pop.nome_urna}</span>
						<span class="text-[10px] text-slate-400 font-mono">({pop.sigla_partido}/{pop.uf})</span>
					</button>
				{/each}
			</div>
		</div>
	{/if}

	<!-- Painel Comparativo Completo quando ambos os parlamentares estão carregados -->
	{#if politico1 && politico2}
		<div class="space-y-6">
			<!-- Tabela Comparativa Estruturada -->
			<div class="overflow-x-auto rounded-3xl border border-slate-800 bg-slate-900 shadow-2xl">
				<table class="w-full text-left text-xs text-slate-300 border-collapse">
					<thead>
						<tr class="bg-slate-950 border-b border-slate-800">
							<th class="p-4 sm:p-5 w-1/3 text-xs font-bold text-slate-400 uppercase tracking-wider">
								Dimensão Auditada
							</th>
							<th class="p-4 sm:p-5 w-1/3 text-center border-l border-slate-800 bg-emerald-950/20">
								<div class="text-sm sm:text-base font-black text-emerald-300">{politico1.nome_urna}</div>
								<div class="text-[11px] text-slate-400 font-mono mt-0.5">{politico1.partido} • {politico1.uf}</div>
							</th>
							<th class="p-4 sm:p-5 w-1/3 text-center border-l border-slate-800 bg-cyan-950/20">
								<div class="text-sm sm:text-base font-black text-cyan-300">{politico2.nome_urna}</div>
								<div class="text-[11px] text-slate-400 font-mono mt-0.5">{politico2.partido} • {politico2.uf}</div>
							</th>
						</tr>
					</thead>

					<tbody class="divide-y divide-slate-800/60 font-sans">
						<!-- 1. Score de Integridade -->
						<tr class="hover:bg-slate-850/50 transition-colors">
							<td class="p-4 font-semibold text-white">
								<div class="flex items-center gap-2">
									<span>🛡️ Score de Integridade</span>
								</div>
								<span class="text-[11px] text-slate-400 font-normal">Índice sintético de conformidade e auditoria</span>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								{#if politico1.score_integridade != null}
									<span class="text-xl font-black font-mono" style="color: {politico1.cor_risco_hex || '#10b981'};">
										{politico1.score_integridade}
									</span>
									<span class="text-xs text-slate-500">/100</span>
									<div class="text-[10px] font-bold uppercase mt-0.5" style="color: {politico1.cor_risco_hex || '#10b981'};">
										Risco {politico1.nivel_risco || 'MÍNIMO'}
									</div>
								{:else}
									<span class="text-slate-500">Não calculado</span>
								{/if}
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								{#if politico2.score_integridade != null}
									<span class="text-xl font-black font-mono" style="color: {politico2.cor_risco_hex || '#10b981'};">
										{politico2.score_integridade}
									</span>
									<span class="text-xs text-slate-500">/100</span>
									<div class="text-[10px] font-bold uppercase mt-0.5" style="color: {politico2.cor_risco_hex || '#10b981'};">
										Risco {politico2.nivel_risco || 'MÍNIMO'}
									</div>
								{:else}
									<span class="text-slate-500">Não calculado</span>
								{/if}
							</td>
						</tr>

						<!-- 2. Gastos Totais CEAP -->
						<tr class="hover:bg-slate-850/50 transition-colors">
							<td class="p-4 font-semibold text-white">
								<div class="flex items-center gap-2">
									<span>🧾 Cota Parlamentar (CEAP)</span>
								</div>
								<span class="text-[11px] text-slate-400 font-normal">Total acumulado de reembolsos oficiais</span>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								<div class="text-base sm:text-lg font-black font-mono text-emerald-400">
									{formatarMoeda(politico1.resumo_financeiro.total_gasto_ceap)}
								</div>
								<div class="text-[11px] text-slate-400">
									{politico1.resumo_financeiro.total_notas_ceap} notas fiscais
								</div>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								<div class="text-base sm:text-lg font-black font-mono text-cyan-400">
									{formatarMoeda(politico2.resumo_financeiro.total_gasto_ceap)}
								</div>
								<div class="text-[11px] text-slate-400">
									{politico2.resumo_financeiro.total_notas_ceap} notas fiscais
								</div>
							</td>
						</tr>

						<!-- 3. Média Mensal CEAP -->
						<tr class="hover:bg-slate-850/50 transition-colors">
							<td class="p-4 font-semibold text-white">
								<span>Média Mensal Estimada CEAP</span>
							</td>
							<td class="p-4 text-center border-l border-slate-800 font-mono font-bold text-slate-200">
								{formatarMoeda(politico1.resumo_financeiro.media_mensal_ceap)}/mês
							</td>
							<td class="p-4 text-center border-l border-slate-800 font-mono font-bold text-slate-200">
								{formatarMoeda(politico2.resumo_financeiro.media_mensal_ceap)}/mês
							</td>
						</tr>

						<!-- 4. Gastos Fora da UF -->
						<tr class="hover:bg-slate-850/50 transition-colors">
							<td class="p-4 font-semibold text-white">
								<span>Despesas Fora da UF de Origem</span>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								<span class="font-mono font-bold {politico1.resumo_financeiro.total_fora_uf > 0 ? 'text-rose-400' : 'text-slate-300'}">
									{formatarMoeda(politico1.resumo_financeiro.total_fora_uf)}
								</span>
								<div class="text-[10px] text-slate-500">{politico1.resumo_financeiro.notas_fora_uf} notas fora de {politico1.uf}</div>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								<span class="font-mono font-bold {politico2.resumo_financeiro.total_fora_uf > 0 ? 'text-rose-400' : 'text-slate-300'}">
									{formatarMoeda(politico2.resumo_financeiro.total_fora_uf)}
								</span>
								<div class="text-[10px] text-slate-500">{politico2.resumo_financeiro.notas_fora_uf} notas fora de {politico2.uf}</div>
							</td>
						</tr>

						<!-- 5. Principal Categoria de Gasto -->
						<tr class="hover:bg-slate-850/50 transition-colors">
							<td class="p-4 font-semibold text-white">
								<span>Principal Rubrica de Gasto</span>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								<div class="text-xs font-bold text-slate-200">
									{politico1.resumo_financeiro.categoria_mais_gasta || 'Sem registros'}
								</div>
								<div class="text-[11px] font-mono text-emerald-400">
									{formatarMoeda(politico1.resumo_financeiro.valor_categoria_mais_gasta)}
								</div>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								<div class="text-xs font-bold text-slate-200">
									{politico2.resumo_financeiro.categoria_mais_gasta || 'Sem registros'}
								</div>
								<div class="text-[11px] font-mono text-cyan-400">
									{formatarMoeda(politico2.resumo_financeiro.valor_categoria_mais_gasta)}
								</div>
							</td>
						</tr>

						<!-- 6. Emendas Parlamentares (Empenhado / Pago) -->
						<tr class="hover:bg-slate-850/50 transition-colors">
							<td class="p-4 font-semibold text-white">
								<div class="flex items-center gap-2">
									<span>💰 Emendas Parlamentares</span>
								</div>
								<span class="text-[11px] text-slate-400 font-normal">Alocação orçamentária (Siop / Transparência)</span>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								{#if politico1.emendas && politico1.emendas.length > 0}
									<div class="font-mono font-bold text-emerald-400 text-sm">
										Pago: {formatarMoeda(calcEmendasPago(politico1))}
									</div>
									<div class="text-[10px] text-slate-400">
										Empenhado: {formatarMoeda(calcEmendasEmpenhado(politico1))} ({politico1.emendas.length} emendas)
									</div>
								{:else}
									<span class="text-slate-500">Sem emendas registradas</span>
								{/if}
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								{#if politico2.emendas && politico2.emendas.length > 0}
									<div class="font-mono font-bold text-cyan-400 text-sm">
										Pago: {formatarMoeda(calcEmendasPago(politico2))}
									</div>
									<div class="text-[10px] text-slate-400">
										Empenhado: {formatarMoeda(calcEmendasEmpenhado(politico2))} ({politico2.emendas.length} emendas)
									</div>
								{:else}
									<span class="text-slate-500">Sem emendas registradas</span>
								{/if}
							</td>
						</tr>

						<!-- 7. Patrimônio Declarado -->
						<tr class="hover:bg-slate-850/50 transition-colors">
							<td class="p-4 font-semibold text-white">
								<div class="flex items-center gap-2">
									<span>💼 Patrimônio Declarado (TSE)</span>
								</div>
								<span class="text-[11px] text-slate-400 font-normal">Bens informados à Justiça Eleitoral</span>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								<div class="text-sm sm:text-base font-black font-mono text-indigo-400">
									{formatarMoeda(politico1.resumo_financeiro.total_bens_declarados)}
								</div>
								<div class="text-[10px] text-slate-500">{politico1.historico_bens?.length || 0} itens cadastrados</div>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								<div class="text-sm sm:text-base font-black font-mono text-indigo-400">
									{formatarMoeda(politico2.resumo_financeiro.total_bens_declarados)}
								</div>
								<div class="text-[10px] text-slate-500">{politico2.historico_bens?.length || 0} itens cadastrados</div>
							</td>
						</tr>

						<!-- 8. Alertas & Anomalias de Auditoria -->
						<tr class="hover:bg-slate-850/50 transition-colors">
							<td class="p-4 font-semibold text-white">
								<div class="flex items-center gap-2">
									<span>🚨 Alertas de Auditoria Cívica</span>
								</div>
								<span class="text-[11px] text-slate-400 font-normal">Irregularidades ou incompatibilidades sinalizadas</span>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								{#if totalAlertas(politico1) > 0}
									<span class="px-2.5 py-1 rounded-full text-xs font-bold font-mono bg-rose-500/20 text-rose-300 border border-rose-500/40 inline-block">
										⚠️ {totalAlertas(politico1)} alertas ativos
									</span>
								{:else}
									<span class="px-2.5 py-1 rounded-full text-xs font-bold font-mono bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 inline-block">
										✓ Sem alertas críticos
									</span>
								{/if}
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								{#if totalAlertas(politico2) > 0}
									<span class="px-2.5 py-1 rounded-full text-xs font-bold font-mono bg-rose-500/20 text-rose-300 border border-rose-500/40 inline-block">
										⚠️ {totalAlertas(politico2)} alertas ativos
									</span>
								{:else}
									<span class="px-2.5 py-1 rounded-full text-xs font-bold font-mono bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 inline-block">
										✓ Sem alertas críticos
									</span>
								{/if}
							</td>
						</tr>

						<!-- Links de Ação Individual -->
						<tr class="bg-slate-950 print:hidden">
							<td class="p-4 font-bold text-slate-400">Ações Detalhadas</td>
							<td class="p-4 text-center border-l border-slate-800">
								<a
									href={`/politicos/${politico1.id}`}
									class="inline-block px-3 py-1.5 bg-emerald-600/20 hover:bg-emerald-600/30 border border-emerald-500/40 text-emerald-300 rounded-lg text-xs font-semibold transition-colors"
								>
									Ver Perfil Completo →
								</a>
							</td>
							<td class="p-4 text-center border-l border-slate-800">
								<a
									href={`/politicos/${politico2.id}`}
									class="inline-block px-3 py-1.5 bg-cyan-600/20 hover:bg-cyan-600/30 border border-cyan-500/40 text-cyan-300 rounded-lg text-xs font-semibold transition-colors"
								>
									Ver Perfil Completo →
								</a>
							</td>
						</tr>
					</tbody>
				</table>
			</div>

			<!-- Rodapé de Auditoria e Fontes -->
			<div class="p-4 bg-slate-900 border border-slate-800 rounded-2xl text-[11px] text-slate-500 flex flex-col sm:flex-row items-center justify-between gap-2">
				<span>Fontes públicas: Tribunal Superior Eleitoral (TSE), Câmara dos Deputados (CEAP), Senado Federal e Siop/Planejamento.</span>
				<span class="font-mono text-slate-400">Radar Cívico • Controle Social da Democracia</span>
			</div>
		</div>
	{:else if !loading1 && !loading2}
		<!-- Estado Inicial: Nenhum ou apenas um parlamentar selecionado -->
		<div class="py-16 text-center bg-slate-900/60 border border-dashed border-slate-800 rounded-3xl space-y-4">
			<span class="text-4xl block">🔍</span>
			<h3 class="text-base font-bold text-white">Selecione dois parlamentares para iniciar a comparação</h3>
			<p class="text-xs text-slate-400 max-w-md mx-auto">
				Utilize as caixas de busca acima para pesquisar e escolher os parlamentares que deseja contrastar lado a lado.
			</p>
		</div>
	{/if}
</div>

<style>
	@media print {
		@page {
			size: A4 landscape;
			margin: 10mm;
		}
		:global(body) {
			background: #ffffff !important;
			color: #000000 !important;
		}
		:global(nav),
		:global(header),
		:global(footer),
		button,
		input {
			display: none !important;
		}
		:global(.bg-slate-900),
		:global(.bg-slate-950) {
			background: #ffffff !important;
			color: #000000 !important;
			border-color: #cbd5e1 !important;
		}
		:global(.text-white),
		:global(.text-slate-200),
		:global(.text-slate-300) {
			color: #0f172a !important;
		}
		:global(.text-slate-400),
		:global(.text-slate-500) {
			color: #475569 !important;
		}
	}
</style>
