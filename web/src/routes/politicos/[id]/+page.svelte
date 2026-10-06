<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import MapaDespesas from '$lib/components/MapaDespesas.svelte';
	import type { PoliticoDetalheResponse, PoliticoDespesasGeoResponse } from '$lib/types';

	const id = $page.params.id;

	let loading = true;
	let erro: string | null = null;
	let politico: PoliticoDetalheResponse | null = null;
	let geoData: PoliticoDespesasGeoResponse | null = null;
	let abaAtiva: 'mapa' | 'categorias' | 'despesas' | 'eleitoral' = 'mapa';

	async function carregarDados() {
		loading = true;
		erro = null;
		try {
			const [resDetalhe, resGeo] = await Promise.all([
				fetch(`/api/politicos/${id}`),
				fetch(`/api/politicos/${id}/despesas-geo`)
			]);

			if (!resDetalhe.ok) {
				if (resDetalhe.status === 404) {
					erro = 'Parlamentar não encontrado no banco de dados.';
				} else {
					erro = 'Erro ao consultar detalhes do parlamentar.';
				}
				return;
			}

			politico = await resDetalhe.json();

			if (resGeo.ok) {
				geoData = await resGeo.json();
			} else {
				geoData = {
					politico_id: Number(id),
					politico_nome: politico?.nome_urna || '',
					politico_uf: politico?.uf || 'BR',
					total_despesas_geo: 0,
					total_valor_geo: 0,
					despesas_fora_uf_total: 0,
					despesas_fora_uf_valor: 0,
					pontos: []
				};
			}
		} catch (err) {
			erro = 'Falha de conexão com a API do Radar Cívico.';
		} finally {
			loading = false;
		}
	}

	function formatarMoeda(val: number): string {
		return val.toLocaleString('pt-BR', { style: 'currency', currency: 'BRL' });
	}

	function formatarData(dataStr: string | null): string {
		if (!dataStr) return '-';
		const parts = dataStr.split('-');
		if (parts.length === 3) {
			return `${parts[2]}/${parts[1]}/${parts[0]}`;
		}
		return dataStr;
	}

	function formatarCnpjCpf(doc: string): string {
		const clean = doc.replace(/\D/g, '');
		if (clean.length === 14) {
			return clean.replace(/^(\d{2})(\d{3})(\d{3})(\d{4})(\d{2})$/, '$1.$2.$3/$4-$5');
		}
		if (clean.length === 11) {
			return clean.replace(/^(\d{3})(\d{3})(\d{3})(\d{2})$/, '$1.$2.$3-$4');
		}
		return doc;
	}

	function getIniciais(nome: string): string {
		const parts = nome.trim().split(/\s+/);
		if (parts.length === 1) return parts[0].substring(0, 2).toUpperCase();
		return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
	}

	function getPartidoColor(partido: string): string {
		const p = (partido || '').toUpperCase();
		if (['PT', 'PCdoB', 'PSOL', 'PV', 'REDE'].includes(p)) return 'from-rose-600 to-red-700 border-rose-500/40 text-rose-300';
		if (['PL', 'PP', 'REPUBLICANOS'].includes(p)) return 'from-blue-600 to-indigo-700 border-blue-500/40 text-blue-300';
		if (['MDB', 'PSD', 'UNIÃO', 'PODEMOS', 'PODE'].includes(p)) return 'from-emerald-600 to-teal-700 border-emerald-500/40 text-emerald-300';
		if (['PSDB', 'CIDADANIA'].includes(p)) return 'from-cyan-600 to-blue-600 border-cyan-500/40 text-cyan-300';
		if (['PDT', 'PSB', 'SOLIDARIEDADE'].includes(p)) return 'from-amber-600 to-orange-700 border-amber-500/40 text-amber-300';
		if (['NOVO'].includes(p)) return 'from-orange-600 to-amber-600 border-orange-500/40 text-orange-300';
		return 'from-slate-700 to-slate-800 border-slate-600 text-slate-300';
	}

	onMount(() => {
		carregarDados();
	});
</script>

<svelte:head>
	<title>{politico ? `${politico.nome_urna} (${politico.partido}-${politico.uf}) | Radar Cívico` : 'Perfil Parlamentar | Radar Cívico'}</title>
</svelte:head>

<div class="space-y-6">
	<!-- Navegação de Retorno -->
	<div class="flex items-center justify-between">
		<a
			href="/politicos"
			class="text-xs font-medium text-slate-400 hover:text-emerald-400 flex items-center gap-1.5 transition-colors group"
		>
			<svg class="w-4 h-4 transform group-hover:-translate-x-1 transition-transform" fill="none" viewBox="0 0 24 24" stroke="currentColor">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
			</svg>
			<span>Voltar ao Catálogo de Políticos</span>
		</a>

		{#if politico?.cpf_mascarado}
			<a
				href="/dossie/cpf/{politico.cpf_mascarado.replace(/\D/g, '')}"
				class="text-xs font-semibold px-3 py-1.5 rounded-lg bg-indigo-600/20 border border-indigo-500/40 text-indigo-300 hover:bg-indigo-600/30 transition-colors flex items-center gap-1.5"
			>
				<span>Ver Dossiê Completo de Vínculos (CPF)</span>
				<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 5l7 7m0 0l-7 7m7-7H3" />
				</svg>
			</a>
		{/if}
	</div>

	{#if loading}
		<div class="py-24 flex flex-col justify-center items-center gap-3">
			<div class="w-10 h-10 border-3 border-emerald-500 border-t-transparent rounded-full animate-spin"></div>
			<p class="text-sm text-slate-300 font-medium">Carregando perfil e mapeando despesas georreferenciadas...</p>
			<p class="text-xs text-slate-500">Agregando notas fiscais da CEAP e coordenadas dos estabelecimentos</p>
		</div>
	{:else if erro}
		<div class="p-8 text-center bg-rose-500/10 border border-rose-500/30 rounded-2xl text-rose-300 text-sm max-w-lg mx-auto space-y-3">
			<p class="font-semibold">{erro}</p>
			<a href="/politicos" class="inline-block px-4 py-2 bg-slate-800 hover:bg-slate-700 text-xs text-white rounded-lg transition-colors font-medium">
				Voltar para lista de políticos
			</a>
		</div>
	{:else if politico}
		<!-- Header Principal do Parlamentar -->
		<section class="bg-slate-900 border border-slate-800 rounded-3xl p-6 shadow-xl relative overflow-hidden">
			<!-- Detalhe Gradiente no Topo -->
			<div class="absolute top-0 left-0 right-0 h-1.5 bg-gradient-to-r {getPartidoColor(politico.partido)}"></div>

			<div class="flex flex-col md:flex-row md:items-center justify-between gap-6">
				<!-- Avatar / Foto + Dados Principais -->
				<div class="flex items-start sm:items-center gap-5">
					{#if politico.foto_base64}
						<img
							src={`data:${politico.foto_mime || 'image/jpeg'};base64,${politico.foto_base64}`}
							alt={politico.nome_urna}
							class="w-20 h-20 sm:w-24 sm:h-24 rounded-2xl object-cover border-2 border-slate-700 bg-slate-950 shadow-lg flex-shrink-0"
						/>
					{:else}
						<div class="w-20 h-20 sm:w-24 sm:h-24 rounded-2xl bg-gradient-to-br {getPartidoColor(politico.partido)} flex items-center justify-center font-extrabold text-2xl sm:text-3xl text-white shadow-xl flex-shrink-0 border-2 border-slate-700">
							{getIniciais(politico.nome_urna || politico.nome_completo)}
						</div>
					{/if}

					<div class="space-y-1.5">
						<div class="flex flex-wrap items-center gap-2">
							<span class="px-2.5 py-0.5 rounded-lg text-xs font-bold bg-slate-800 border border-slate-700 text-white font-mono shadow-sm">
								{politico.partido || 'SEM PARTIDO'}
							</span>
							<span class="px-2.5 py-0.5 rounded-lg text-xs font-bold bg-slate-800 border border-slate-700 text-emerald-400 font-mono shadow-sm">
								{politico.uf || 'BR'}
							</span>
							<span class="px-2.5 py-0.5 rounded-lg text-xs font-medium bg-slate-800/80 border border-slate-700/60 text-slate-300">
								{politico.cargo || 'Deputado Federal'}
							</span>
							{#if politico.municipio}
								<span class="text-xs text-slate-400">
									• {politico.municipio}
								</span>
							{/if}
						</div>

						<h1 class="text-2xl sm:text-3xl font-black text-white tracking-tight">
							{politico.nome_urna}
						</h1>

						<p class="text-sm text-slate-300 font-medium">
							{politico.nome_completo}
						</p>

						<div class="flex flex-wrap items-center gap-3 text-xs text-slate-400 pt-1 font-mono">
							{#if politico.cpf_mascarado}
								<span>CPF: <strong class="text-slate-300">{politico.cpf_mascarado}</strong></span>
							{/if}
							{#if politico.ocupacao}
								<span class="font-sans">• Ocupação: <strong class="text-slate-300 font-sans">{politico.ocupacao}</strong></span>
							{/if}
							{#if politico.grau_instrucao}
								<span class="font-sans">• {politico.grau_instrucao}</span>
							{/if}
						</div>
					</div>
				</div>

				<!-- Indicador Rápido de Localização do Mandato -->
				<div class="flex flex-col sm:items-end justify-center bg-slate-950/70 border border-slate-800 rounded-2xl p-4 min-w-[200px]">
					<span class="text-xs text-slate-400 uppercase tracking-wider font-semibold">Base Parlamentar</span>
					<div class="text-lg font-bold text-white mt-0.5">
						{politico.uf} - Brasil
					</div>
					<div class="text-xs text-slate-400 mt-1 flex items-center gap-1">
						<span class="w-2 h-2 rounded-full bg-emerald-400"></span>
						<span>Gabinete em Exercício</span>
					</div>
				</div>
			</div>
		</section>

		<!-- Cards de Métricas e KPIs Financeiros -->
		<section class="grid grid-cols-2 lg:grid-cols-4 gap-4">
			<!-- Total CEAP -->
			<div class="bg-slate-900 border border-slate-800 rounded-2xl p-4 shadow-md flex flex-col justify-between">
				<span class="text-xs font-semibold text-slate-400 uppercase tracking-wider">Cota Parlamentar (CEAP)</span>
				<div class="my-2">
					<div class="text-xl sm:text-2xl font-black text-emerald-400 font-mono">
						{formatarMoeda(politico.resumo_financeiro.total_gasto_ceap)}
					</div>
					<span class="text-xs text-slate-400">
						{politico.resumo_financeiro.total_notas_ceap} notas fiscais emitidas
					</span>
				</div>
				<div class="pt-2 border-t border-slate-800/80 text-[11px] text-slate-400 flex justify-between">
					<span>Média Estimada:</span>
					<strong class="text-slate-300 font-mono">{formatarMoeda(politico.resumo_financeiro.media_mensal_ceap)}/mês</strong>
				</div>
			</div>

			<!-- Despesas Fora da UF -->
			<div class="bg-slate-900 border {politico.resumo_financeiro.total_fora_uf > 0 ? 'border-rose-500/30 bg-rose-950/10' : 'border-slate-800'} rounded-2xl p-4 shadow-md flex flex-col justify-between">
				<div class="flex items-center justify-between">
					<span class="text-xs font-semibold text-slate-400 uppercase tracking-wider">Gastos Fora da UF</span>
					{#if politico.resumo_financeiro.total_fora_uf > 0}
						<span class="w-2 h-2 rounded-full bg-rose-500 animate-ping"></span>
					{/if}
				</div>
				<div class="my-2">
					<div class="text-xl sm:text-2xl font-black {politico.resumo_financeiro.total_fora_uf > 0 ? 'text-rose-400' : 'text-slate-300'} font-mono">
						{formatarMoeda(politico.resumo_financeiro.total_fora_uf)}
					</div>
					<span class="text-xs text-slate-400">
						{politico.resumo_financeiro.notas_fora_uf} notas fora da base ({politico.uf})
					</span>
				</div>
				<div class="pt-2 border-t border-slate-800/80 text-[11px] text-slate-400 flex justify-between">
					<span>Distância & Viagens:</span>
					<strong class="{politico.resumo_financeiro.notas_fora_uf > 0 ? 'text-rose-300' : 'text-emerald-400'}">
						{politico.resumo_financeiro.notas_fora_uf > 0 ? 'Alerta Ativo ⚠️' : 'Dentro do Padrão'}
					</strong>
				</div>
			</div>

			<!-- Maior Categoria de Gastos -->
			<div class="bg-slate-900 border border-slate-800 rounded-2xl p-4 shadow-md flex flex-col justify-between">
				<span class="text-xs font-semibold text-slate-400 uppercase tracking-wider">Principal Despesa</span>
				<div class="my-2">
					<div class="text-sm font-bold text-cyan-300 line-clamp-2 leading-tight" title={politico.resumo_financeiro.categoria_mais_gasta || 'Nenhuma'}>
						{politico.resumo_financeiro.categoria_mais_gasta || 'Sem registros'}
					</div>
					<div class="text-lg font-black text-white font-mono mt-1">
						{formatarMoeda(politico.resumo_financeiro.valor_categoria_mais_gasta)}
					</div>
				</div>
				<div class="pt-2 border-t border-slate-800/80 text-[11px] text-slate-400 flex justify-between">
					<span>Concentração:</span>
					<span class="text-slate-300 font-semibold">
						{politico.resumo_financeiro.total_gasto_ceap > 0
							? `${((politico.resumo_financeiro.valor_categoria_mais_gasta / politico.resumo_financeiro.total_gasto_ceap) * 100).toFixed(1)}%`
							: '0%'}
					</span>
				</div>
			</div>

			<!-- Bens Declarados no TSE -->
			<div class="bg-slate-900 border border-slate-800 rounded-2xl p-4 shadow-md flex flex-col justify-between">
				<span class="text-xs font-semibold text-slate-400 uppercase tracking-wider">Patrimônio Declarado</span>
				<div class="my-2">
					<div class="text-xl sm:text-2xl font-black text-indigo-400 font-mono">
						{formatarMoeda(politico.resumo_financeiro.total_bens_declarados)}
					</div>
					<span class="text-xs text-slate-400">
						Declarado à Justiça Eleitoral
					</span>
				</div>
				<div class="pt-2 border-t border-slate-800/80 text-[11px] text-slate-400 flex justify-between">
					<span>Bens Registrados:</span>
					<strong class="text-slate-300 font-mono">{politico.historico_bens?.length || 0} itens</strong>
				</div>
			</div>
		</section>

		<!-- Abas de Navegação do Módulo -->
		<div class="border-b border-slate-800 flex items-center gap-2 overflow-x-auto text-xs font-medium">
			<button
				type="button"
				on:click={() => (abaAtiva = 'mapa')}
				class="pb-3 px-3 transition-colors flex items-center gap-2 border-b-2 font-bold whitespace-nowrap {abaAtiva === 'mapa' ? 'border-emerald-400 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}"
			>
				<svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7" />
				</svg>
				<span>Mapa Interativo de Gastos ({geoData?.pontos?.length || 0})</span>
			</button>

			<button
				type="button"
				on:click={() => (abaAtiva = 'categorias')}
				class="pb-3 px-3 transition-colors flex items-center gap-2 border-b-2 font-bold whitespace-nowrap {abaAtiva === 'categorias' ? 'border-cyan-400 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}"
			>
				<svg class="w-4 h-4 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 3.055A9.001 9.001 0 1020.945 13H11V3.055z" />
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.488 9H15V3.512A9.025 9.025 0 0120.488 9z" />
				</svg>
				<span>Categorias de Despesa ({politico.gastos_por_categoria?.length || 0})</span>
			</button>

			<button
				type="button"
				on:click={() => (abaAtiva = 'despesas')}
				class="pb-3 px-3 transition-colors flex items-center gap-2 border-b-2 font-bold whitespace-nowrap {abaAtiva === 'despesas' ? 'border-indigo-400 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}"
			>
				<svg class="w-4 h-4 text-indigo-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
				</svg>
				<span>Notas Fiscais Recentes ({politico.despesas_recentes?.length || 0})</span>
			</button>

			<button
				type="button"
				on:click={() => (abaAtiva = 'eleitoral')}
				class="pb-3 px-3 transition-colors flex items-center gap-2 border-b-2 font-bold whitespace-nowrap {abaAtiva === 'eleitoral' ? 'border-purple-400 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}"
			>
				<svg class="w-4 h-4 text-purple-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
				</svg>
				<span>Histórico Eleitoral & Bens</span>
			</button>
		</div>

		<!-- Conteúdo das Abas -->
		{#if abaAtiva === 'mapa'}
			<section class="space-y-4">
				{#if geoData && geoData.pontos && geoData.pontos.length > 0}
					<MapaDespesas
						pontos={geoData.pontos}
						politicoNome={politico.nome_urna}
						politicoUf={politico.uf}
					/>
				{:else}
					<div class="p-12 text-center bg-slate-900 border border-slate-800 rounded-2xl space-y-3">
						<span class="text-3xl">🗺️</span>
						<h3 class="text-base font-semibold text-white">Nenhum ponto georreferenciado disponível</h3>
						<p class="text-xs text-slate-400 max-w-md mx-auto">
							Não foram encontradas coordenadas válidas ou notas fiscais com fornecedores cadastrados para este parlamentar.
						</p>
					</div>
				{/if}
			</section>
		{:else if abaAtiva === 'categorias'}
			<!-- Gráfico / Barras de Despesas por Categoria -->
			<section class="bg-slate-900 border border-slate-800 rounded-2xl p-6 shadow-xl space-y-6">
				<div class="flex items-center justify-between pb-3 border-b border-slate-800">
					<div>
						<h3 class="text-base font-bold text-white">Detalhamento dos Gastos por Categoria</h3>
						<p class="text-xs text-slate-400">Classificação oficial de reembolsos da Cota para o Exercício da Atividade Parlamentar (CEAP)</p>
					</div>
					<div class="text-xs text-slate-400 font-mono">
						Total: <strong class="text-emerald-400 font-bold">{formatarMoeda(politico.resumo_financeiro.total_gasto_ceap)}</strong>
					</div>
				</div>

				{#if politico.gastos_por_categoria && politico.gastos_por_categoria.length > 0}
					<div class="space-y-4">
						{#each politico.gastos_por_categoria as cat}
							<div class="space-y-1.5">
								<div class="flex items-center justify-between text-xs">
									<div class="flex items-center gap-2">
										<span class="font-semibold text-slate-200">{cat.categoria}</span>
										<span class="text-slate-500 font-mono">({cat.quantidade} {cat.quantidade === 1 ? 'nota' : 'notas'})</span>
									</div>
									<div class="flex items-center gap-3 font-mono">
										<strong class="text-white">{formatarMoeda(cat.total)}</strong>
										<span class="text-slate-400 w-12 text-right">{cat.percentual.toFixed(1)}%</span>
									</div>
								</div>

								<!-- Barra de Progresso Visual -->
								<div class="w-full h-2.5 bg-slate-950 rounded-full overflow-hidden border border-slate-800/80">
									<div
										class="h-full rounded-full transition-all duration-500 bg-gradient-to-r {cat.categoria.toUpperCase().includes('COMBUST') ? 'from-amber-500 to-yellow-500' : 'from-emerald-500 to-teal-400'}"
										style={`width: ${Math.min(100, Math.max(1, cat.percentual))}%`}
									></div>
								</div>
							</div>
						{/each}
					</div>
				{:else}
					<p class="text-xs text-slate-400 text-center py-6">Nenhuma categoria registrada.</p>
				{/if}
			</section>
		{:else if abaAtiva === 'despesas'}
			<!-- Relação das Notas Fiscais Recentes -->
			<section class="bg-slate-900 border border-slate-800 rounded-2xl p-6 shadow-xl space-y-4">
				<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 pb-3 border-b border-slate-800">
					<div>
						<h3 class="text-base font-bold text-white">Histórico de Notas Fiscais Emitidas</h3>
						<p class="text-xs text-slate-400">Relação das despesas declaradas com link direto para auditoria do CNPJ fornecedor</p>
					</div>
					<span class="text-xs text-slate-400 font-mono">
						Mostrando as últimas <strong>{politico.despesas_recentes?.length || 0}</strong> despesas
					</span>
				</div>

				{#if politico.despesas_recentes && politico.despesas_recentes.length > 0}
					<div class="overflow-x-auto">
						<table class="w-full text-left text-xs">
							<thead>
								<tr class="border-b border-slate-800 text-slate-400 font-semibold uppercase tracking-wider text-[11px]">
									<th class="py-2.5 px-3">Data</th>
									<th class="py-2.5 px-3">Fornecedor / Razão Social</th>
									<th class="py-2.5 px-3">CNPJ / CPF</th>
									<th class="py-2.5 px-3">Categoria</th>
									<th class="py-2.5 px-3 text-right">Valor</th>
									<th class="py-2.5 px-3 text-center">Ações</th>
								</tr>
							</thead>
							<tbody class="divide-y divide-slate-800/60">
								{#each politico.despesas_recentes as despesa}
									<tr class="hover:bg-slate-850/60 transition-colors">
										<td class="py-3 px-3 font-mono text-slate-300 whitespace-nowrap">
											{formatarData(despesa.data_emissao)}
										</td>
										<td class="py-3 px-3 font-medium text-white max-w-[220px] truncate" title={despesa.fornecedor_nome}>
											{despesa.fornecedor_nome}
											{#if despesa.flag_anomalia}
												<span class="ml-1.5 px-1.5 py-0.2 rounded text-[10px] font-bold bg-rose-500/20 text-rose-300 border border-rose-500/40">
													⚠️ Anomalia
												</span>
											{/if}
										</td>
										<td class="py-3 px-3 font-mono text-slate-400 whitespace-nowrap">
											{formatarCnpjCpf(despesa.fornecedor_cnpj_cpf)}
										</td>
										<td class="py-3 px-3 text-cyan-300 max-w-[200px] truncate" title={despesa.categoria_despesa}>
											{despesa.categoria_despesa}
											{#if despesa.detalhes_litros}
												<span class="ml-1 text-[11px] text-amber-300 font-mono">({despesa.detalhes_litros.toFixed(1)} L)</span>
											{/if}
										</td>
										<td class="py-3 px-3 text-right font-mono font-bold text-emerald-400 whitespace-nowrap">
											{formatarMoeda(despesa.valor_liquido)}
										</td>
										<td class="py-3 px-3 text-center whitespace-nowrap">
											{#if despesa.fornecedor_cnpj_cpf.replace(/\D/g, '').length === 14}
												<a
													href="/dossie/cnpj/{despesa.fornecedor_cnpj_cpf.replace(/\D/g, '')}"
													class="px-2.5 py-1 rounded-md bg-indigo-600/20 hover:bg-indigo-600/30 text-indigo-300 border border-indigo-500/40 text-[11px] font-medium transition-colors inline-block"
												>
													Dossiê CNPJ 🔍
												</a>
											{:else}
												<span class="text-slate-600">-</span>
											{/if}
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{:else}
					<p class="text-xs text-slate-400 text-center py-6">Nenhuma despesa recente registrada.</p>
				{/if}
			</section>
		{:else if abaAtiva === 'eleitoral'}
			<!-- Histórico Eleitoral, Bens e Doações -->
			<div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
				<!-- Bens Declarados -->
				<section class="bg-slate-900 border border-slate-800 rounded-2xl p-5 shadow-xl space-y-4">
					<div class="flex items-center justify-between pb-3 border-b border-slate-800">
						<div>
							<h3 class="text-sm font-bold text-white">Declaração de Bens (TSE)</h3>
							<p class="text-xs text-slate-400">Patrimônio declarado à Justiça Eleitoral</p>
						</div>
						<span class="text-xs font-mono font-bold text-indigo-400">
							{formatarMoeda(politico.resumo_financeiro.total_bens_declarados)}
						</span>
					</div>

					{#if politico.historico_bens && politico.historico_bens.length > 0}
						<div class="space-y-2 max-h-[360px] overflow-y-auto pr-1">
							{#each politico.historico_bens as bem}
								<div class="p-3 bg-slate-950/70 border border-slate-800 rounded-xl flex items-center justify-between gap-3 text-xs">
									<div class="space-y-0.5 min-w-0">
										<span class="font-bold text-slate-300">{bem.tipo_bem}</span>
										<p class="text-slate-400 text-[11px] truncate" title={bem.descricao}>{bem.descricao}</p>
									</div>
									<strong class="font-mono text-emerald-400 whitespace-nowrap">{formatarMoeda(bem.valor_declarado)}</strong>
								</div>
							{/each}
						</div>
					{:else}
						<p class="text-xs text-slate-500 text-center py-6">Nenhum bem registrado nas eleições cadastradas.</p>
					{/if}
				</section>

				<!-- Histórico de Candidaturas -->
				<section class="bg-slate-900 border border-slate-800 rounded-2xl p-5 shadow-xl space-y-4">
					<div class="flex items-center justify-between pb-3 border-b border-slate-800">
						<div>
							<h3 class="text-sm font-bold text-white">Candidaturas Registradas</h3>
							<p class="text-xs text-slate-400">Histórico de disputas eleitorais no TSE</p>
						</div>
						<span class="text-xs text-slate-400 font-mono">
							{politico.candidaturas?.length || 0} disputas
						</span>
					</div>

					{#if politico.candidaturas && politico.candidaturas.length > 0}
						<div class="space-y-2.5">
							{#each politico.candidaturas as cand}
								<div class="p-3 bg-slate-950/70 border border-slate-800 rounded-xl flex items-center justify-between gap-2 text-xs">
									<div>
										<div class="flex items-center gap-2">
											<span class="font-bold text-white">{cand.cargo}</span>
											<span class="px-1.5 py-0.2 rounded text-[10px] bg-slate-800 font-mono text-emerald-400 font-bold">{cand.ano_eleicao}</span>
										</div>
										<p class="text-slate-400 text-[11px] mt-0.5">
											{cand.sigla_partido} • {cand.uf} {#if cand.municipio}• {cand.municipio}{/if}
										</p>
									</div>
									<div class="text-right">
										<span class="px-2 py-0.5 rounded text-[10px] font-semibold {cand.situacao_totalizacao?.toUpperCase().includes('ELEITO') ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/40' : 'bg-slate-800 text-slate-400'}">
											{cand.situacao_totalizacao || 'Candidato'}
										</span>
									</div>
								</div>
							{/each}
						</div>
					{:else}
						<p class="text-xs text-slate-500 text-center py-6">Nenhuma candidatura histórica listada.</p>
					{/if}
				</section>
			</div>
		{/if}
	{/if}
</div>
