<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import type {
		ItemPoliticoListagem,
		ListarPoliticosResponse,
		RelatorioDuplicadosResponse,
		ResumoDuplicadosResponse
	} from '$lib/types';

	let loading = true;
	let erro: string | null = null;

	// Estados de Fotos
	let buscandoFotosCards: Record<number, boolean> = {};
	let sincronizandoFotosGeral = false;
	let msgSincronizacao: string | null = null;

	// Estados de Duplicados
	let resumoDuplicados: ResumoDuplicadosResponse | null = null;
	let relatorioDuplicados: RelatorioDuplicadosResponse | null = null;
	let carregandoDuplicados = false;
	let paginaDuplicados = 1;
	let mesclandoIds: Record<number, boolean> = {};
	let mesclandoAutomatico = false;
	let msgAuditoria: string | null = null;

	// Parâmetros de Filtro
	let termoBusca = '';
	let partidoSelecionado = '';
	let ufSelecionada = '';
	let cargoSelecionado = '';
	let anoSelecionado = '';
	let apenasComGastos = false;
	let paginaAtual = 1;
	const limitePorPagina = 24;

	// Modo de Visualização: Cards vs Tabulação vs Duplicados
	let modoVisualizacao: 'cards' | 'tabela' | 'duplicados' = 'cards';

	async function carregarResumoDuplicados() {
		try {
			const res = await fetch('/api/politicos/duplicados/resumo');
			if (res.ok) {
				resumoDuplicados = await res.json();
			}
		} catch (e) {
			console.warn('Erro ao obter resumo de duplicados:', e);
		}
	}

	async function carregarRelatorioDuplicados(p = 1) {
		paginaDuplicados = p;
		carregandoDuplicados = true;
		msgAuditoria = null;
		try {
			const res = await fetch(`/api/politicos/duplicados?page=${paginaDuplicados}&limit=10`);
			if (res.ok) {
				relatorioDuplicados = await res.json();
				if (relatorioDuplicados?.resumo) {
					resumoDuplicados = relatorioDuplicados.resumo;
				}
			}
		} catch (e: any) {
			msgAuditoria = `Erro ao carregar duplicados: ${e?.message || 'Falha de rede'}`;
		} finally {
			carregandoDuplicados = false;
		}
	}

	function mudarParaModoDuplicados() {
		modoVisualizacao = 'duplicados';
		if (!relatorioDuplicados) {
			carregarRelatorioDuplicados(1);
		}
	}

	async function mesclarRegistros(idCanonico: number, idDuplicado: number) {
		if (mesclandoIds[idDuplicado]) return;
		if (
			!confirm(
				`Deseja unificar o cadastro redundante #${idDuplicado} no registro principal #${idCanonico}? As candidaturas, bens e histórico serão consolidados de forma atômica.`
			)
		) {
			return;
		}
		mesclandoIds[idDuplicado] = true;
		try {
			const res = await fetch('/api/politicos/duplicados/mesclar', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ id_canonico: idCanonico, id_duplicado: idDuplicado })
			});
			const data = await res.json();
			if (res.ok && data.sucesso) {
				msgAuditoria = data.mensagem;
				carregarRelatorioDuplicados(paginaDuplicados);
				carregarResumoDuplicados();
				carregarPoliticos(false);
			} else {
				alert(data.mensagem || 'Falha ao unificar registros.');
			}
		} catch (e: any) {
			alert(`Erro: ${e?.message || 'Falha na requisição'}`);
		} finally {
			mesclandoIds[idDuplicado] = false;
		}
	}

	async function unificarAutomaticoTodos() {
		if (mesclandoAutomatico) return;
		if (
			!confirm(
				'Deseja consolidar automaticamente os grupos com 100% de certeza cadastral (mesmo nome completo civil e mesma data de nascimento)?'
			)
		) {
			return;
		}
		mesclandoAutomatico = true;
		msgAuditoria = null;
		try {
			const res = await fetch('/api/politicos/duplicados/mesclar-automatico', {
				method: 'POST'
			});
			const data = await res.json();
			if (res.ok && data.sucesso) {
				msgAuditoria = data.mensagem;
				carregarRelatorioDuplicados(1);
				carregarResumoDuplicados();
				carregarPoliticos(false);
			} else {
				alert(data.mensagem || 'Falha na unificação automática.');
			}
		} catch (e: any) {
			alert(`Erro: ${e?.message || 'Falha na requisição'}`);
		} finally {
			mesclandoAutomatico = false;
		}
	}

	async function navegarParaPolitico(id: number | string) {
		const targetUrl = `/politicos/${id}`;
		try {
			await goto(targetUrl);
		} catch (err) {
			console.warn('goto falhou, redirecionando via window.location:', err);
			window.location.href = targetUrl;
		}
	}

	async function buscarFotoCard(politico: ItemPoliticoListagem, e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		if (buscandoFotosCards[politico.id]) return;
		buscandoFotosCards[politico.id] = true;
		try {
			const res = await fetch(`/api/politicos/${politico.id}/buscar-foto-tse`, {
				method: 'POST'
			});
			const data = await res.json();
			if (res.ok && data.sucesso) {
				if (data.foto_base64) {
					politico.foto_base64 = data.foto_base64;
					politico.foto_mime = data.foto_mime || 'image/jpeg';
				}
				if (data.foto_url) {
					politico.foto_url = data.foto_url;
				}
				dados.politicos = [...dados.politicos];
			} else {
				alert(data.mensagem || 'Não foi possível obter a foto deste político.');
			}
		} catch (err: any) {
			alert(`Erro ao buscar foto: ${err?.message || 'Falha na conexão'}`);
		} finally {
			buscandoFotosCards[politico.id] = false;
		}
	}

	async function sincronizarFotosGeral() {
		if (sincronizandoFotosGeral) return;
		sincronizandoFotosGeral = true;
		msgSincronizacao = null;
		try {
			const res = await fetch('/api/politicos/sincronizar-fotos-camara', { method: 'POST' });
			const data = await res.json();
			if (res.ok && data.sucesso) {
				msgSincronizacao = `Sucesso: ${data.atualizados} fotos de deputados foram vinculadas!`;
				carregarPoliticos(false);
			} else {
				msgSincronizacao = data.mensagem || 'Falha ao sincronizar fotos da Câmara.';
			}
		} catch (err: any) {
			msgSincronizacao = `Erro: ${err?.message || 'Falha na requisição'}`;
		} finally {
			sincronizandoFotosGeral = false;
		}
	}

	// Dados da API
	let dados: ListarPoliticosResponse = {
		total: 0,
		page: 1,
		limit: limitePorPagina,
		total_paginas: 1,
		partidos_disponiveis: [],
		ufs_disponiveis: [],
		cargos_disponiveis: [],
		anos_disponiveis: [],
		politicos: []
	};

	// Lista padrão das 27 UFs caso a API ainda não tenha retornado
	const todasUfs = [
		'AC', 'AL', 'AM', 'AP', 'BA', 'CE', 'DF', 'ES', 'GO', 'MA',
		'MG', 'MS', 'MT', 'PA', 'PB', 'PE', 'PI', 'PR', 'RJ', 'RN',
		'RO', 'RR', 'RS', 'SC', 'SE', 'SP', 'TO'
	];

	async function carregarPoliticos(resetarPagina = false) {
		if (resetarPagina) {
			paginaAtual = 1;
		}
		loading = true;
		erro = null;

		const params = new URLSearchParams();
		params.set('page', paginaAtual.toString());
		params.set('limit', limitePorPagina.toString());
		if (termoBusca.trim()) params.set('q', termoBusca.trim());
		if (partidoSelecionado) params.set('partido', partidoSelecionado);
		if (ufSelecionada) params.set('uf', ufSelecionada);
		if (cargoSelecionado) params.set('cargo', cargoSelecionado);
		if (anoSelecionado) params.set('ano', anoSelecionado);
		if (apenasComGastos) params.set('apenas_com_gastos', 'true');

		try {
			const res = await fetch(`/api/politicos?${params.toString()}`);
			if (res.ok) {
				dados = await res.json();
			} else {
				const errData = await res.json().catch(() => null);
				erro = errData?.error || errData?.mensagem || `Erro HTTP ${res.status}: Não foi possível carregar a lista de políticos.`;
			}
		} catch (err: any) {
			erro = `Erro de comunicação com o servidor: ${err?.message || 'Falha ao consultar parlamentares'}`;
		} finally {
			loading = false;
		}
	}

	function handleBuscar(e?: Event) {
		if (e) e.preventDefault();
		carregarPoliticos(true);
	}

	function limparFiltros() {
		termoBusca = '';
		partidoSelecionado = '';
		ufSelecionada = '';
		cargoSelecionado = '';
		anoSelecionado = '';
		apenasComGastos = false;
		carregarPoliticos(true);
	}

	function mudarPagina(novaPagina: number) {
		if (novaPagina >= 1 && novaPagina <= dados.total_paginas && novaPagina !== paginaAtual) {
			paginaAtual = novaPagina;
			carregarPoliticos(false);
			window.scrollTo({ top: 0, behavior: 'smooth' });
		}
	}

	function formatarMoeda(val: number): string {
		return val.toLocaleString('pt-BR', { style: 'currency', currency: 'BRL' });
	}

	function getIniciais(nome: string): string {
		const parts = nome.trim().split(/\s+/);
		if (parts.length === 1) return parts[0].substring(0, 2).toUpperCase();
		return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
	}

	// Cor temática sutil para partido
	function getPartidoColor(partido: string): string {
		const p = partido.toUpperCase();
		if (['PT', 'PCdoB', 'PSOL', 'PV', 'REDE'].includes(p)) return 'from-rose-600 to-red-700 border-rose-500/40 text-rose-300';
		if (['PL', 'PP', 'REPUBLICANOS'].includes(p)) return 'from-blue-600 to-indigo-700 border-blue-500/40 text-blue-300';
		if (['MDB', 'PSD', 'UNIÃO', 'PODEMOS', 'PODE'].includes(p)) return 'from-emerald-600 to-teal-700 border-emerald-500/40 text-emerald-300';
		if (['PSDB', 'CIDADANIA'].includes(p)) return 'from-cyan-600 to-blue-600 border-cyan-500/40 text-cyan-300';
		if (['PDT', 'PSB', 'SOLIDARIEDADE'].includes(p)) return 'from-amber-600 to-orange-700 border-amber-500/40 text-amber-300';
		if (['NOVO'].includes(p)) return 'from-orange-600 to-amber-600 border-orange-500/40 text-orange-300';
		return 'from-slate-700 to-slate-800 border-slate-600 text-slate-300';
	}

	onMount(() => {
		carregarPoliticos(false);
		carregarResumoDuplicados();
	});
</script>

<svelte:head>
	<title>Políticos & Parlamentares | Radar Cívico</title>
</svelte:head>

<div class="space-y-8">
	<!-- Cabeçalho e Introdução -->
	<div class="flex flex-col md:flex-row md:items-end justify-between gap-4 pb-2 border-b border-slate-800">
		<div>
			<div class="flex items-center gap-2 mb-2">
				<span class="px-2.5 py-0.5 rounded-full text-xs font-semibold bg-emerald-500/10 border border-emerald-500/30 text-emerald-400">
					Módulo de Monitoramento Parlamentar
				</span>
			</div>
			<h1 class="text-2xl sm:text-3xl font-extrabold text-white tracking-tight">
				Consulta & Perfil de Políticos
			</h1>
			<p class="text-sm text-slate-400 mt-1 max-w-2xl">
				Explore mandatos, histórico de gastos com a Cota Parlamentar (CEAP), georreferenciamento de despesas e alertas analíticos.
			</p>
		</div>

		<div class="flex flex-col sm:flex-row items-start sm:items-center gap-2.5">
			{#if msgSincronizacao}
				<span class="text-xs px-2.5 py-1 rounded-lg bg-emerald-500/10 border border-emerald-500/30 text-emerald-300">
					{msgSincronizacao}
				</span>
			{/if}

			{#if dados.total > 0}
				<div class="flex items-center gap-2 bg-slate-900 border border-slate-800 px-3.5 py-2 rounded-xl text-xs shadow-sm">
					<span class="w-2 h-2 rounded-full bg-emerald-400"></span>
					<span class="text-slate-400">Cadastrados:</span>
					<strong class="text-white font-mono text-sm">{dados.total}</strong>
					<span class="text-slate-500">parlamentares</span>
				</div>

				<button
					type="button"
					on:click={sincronizarFotosGeral}
					disabled={sincronizandoFotosGeral}
					class="px-3 py-2 rounded-xl bg-slate-800 hover:bg-slate-750 text-slate-200 border border-slate-700 hover:border-emerald-500/40 text-xs font-medium flex items-center gap-1.5 transition-colors disabled:opacity-50 shadow-sm"
					title="Vincula automaticamente as fotos públicas oficiais da Câmara dos Deputados no banco local"
				>
					{#if sincronizandoFotosGeral}
						<div class="w-3.5 h-3.5 border-2 border-emerald-400 border-t-transparent rounded-full animate-spin"></div>
						<span>Sincronizando fotos...</span>
					{:else}
						<span>📸 Sincronizar Fotos</span>
					{/if}
				</button>
			{/if}
		</div>
	</div>

	<!-- Formulário de Busca e Filtros Avançados -->
	<section class="bg-slate-900/90 border border-slate-800 rounded-2xl p-5 shadow-lg space-y-4">
		<form on:submit={handleBuscar} class="space-y-4">
			<!-- Linha 1: Input de Busca Textual -->
			<div class="relative">
				<div class="absolute inset-y-0 left-0 pl-3.5 flex items-center pointer-events-none text-slate-400">
					<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
					</svg>
				</div>
				<input
					type="text"
					bind:value={termoBusca}
					placeholder="Buscar por nome parlamentar ou nome completo (ex: Nikolas, Tabata, Boulos, Lira)..."
					class="w-full pl-11 pr-28 py-3 bg-slate-950 border border-slate-700/80 rounded-xl text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/50 focus:border-emerald-500 text-sm transition-all shadow-inner"
				/>
				<button
					type="submit"
					class="absolute right-2 top-2 bottom-2 px-4 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg font-medium text-xs shadow-sm transition-colors flex items-center gap-1.5"
				>
					<span>Buscar</span>
				</button>
			</div>

			<!-- Linha 2: Filtros Dropdowns (Partido, UF, Cargo, Ano, Checkbox CEAP) -->
			<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-3 text-xs">
				<!-- Filtro por Partido -->
				<div>
					<label for="filtro-partido" class="block text-slate-400 font-medium mb-1.5">Partido Político:</label>
					<select
						id="filtro-partido"
						bind:value={partidoSelecionado}
						on:change={() => carregarPoliticos(true)}
						class="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:outline-none focus:ring-1 focus:ring-emerald-500 focus:border-emerald-500"
					>
						<option value="">Todos os Partidos</option>
						{#each dados.partidos_disponiveis.length ? dados.partidos_disponiveis : ['PL', 'PT', 'UNIÃO', 'PP', 'MDB', 'PSD', 'REPUBLICANOS', 'PSB', 'PDT', 'PSOL', 'PODE', 'PSDB', 'PCdoB', 'PV', 'NOVO', 'AVANTE', 'SOLIDARIEDADE', 'CIDADANIA', 'PRD', 'REDE'] as partido}
							<option value={partido}>{partido}</option>
						{/each}
					</select>
				</div>

				<!-- Filtro por Estado / UF -->
				<div>
					<label for="filtro-uf" class="block text-slate-400 font-medium mb-1.5">Estado / UF de Atuação:</label>
					<select
						id="filtro-uf"
						bind:value={ufSelecionada}
						on:change={() => carregarPoliticos(true)}
						class="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:outline-none focus:ring-1 focus:ring-emerald-500 focus:border-emerald-500"
					>
						<option value="">Todos os Estados (UF)</option>
						{#each dados.ufs_disponiveis.length ? dados.ufs_disponiveis : todasUfs as uf}
							<option value={uf}>{uf}</option>
						{/each}
					</select>
				</div>

				<!-- Filtro por Cargo -->
				<div>
					<label for="filtro-cargo" class="block text-slate-400 font-medium mb-1.5">Cargo / Mandato:</label>
					<select
						id="filtro-cargo"
						bind:value={cargoSelecionado}
						on:change={() => carregarPoliticos(true)}
						class="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:outline-none focus:ring-1 focus:ring-emerald-500 focus:border-emerald-500"
					>
						<option value="">Todos os Cargos</option>
						{#each dados.cargos_disponiveis.length ? dados.cargos_disponiveis : ['PRESIDENTE', 'VICE-PRESIDENTE', 'GOVERNADOR', 'VICE-GOVERNADOR', 'SENADOR', 'DEPUTADO FEDERAL', 'DEPUTADO ESTADUAL', 'DEPUTADO DISTRITAL', 'PREFEITO', 'VICE-PREFEITO', 'VEREADOR'] as cargo}
							<option value={cargo}>{cargo}</option>
						{/each}
					</select>
				</div>

				<!-- Filtro por Ano da Eleição -->
				<div>
					<label for="filtro-ano" class="block text-slate-400 font-medium mb-1.5">Ano da Eleição:</label>
					<select
						id="filtro-ano"
						bind:value={anoSelecionado}
						on:change={() => carregarPoliticos(true)}
						class="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:outline-none focus:ring-1 focus:ring-emerald-500 focus:border-emerald-500"
					>
						<option value="">Todos os Anos</option>
						{#each dados.anos_disponiveis && dados.anos_disponiveis.length ? dados.anos_disponiveis : [2026, 2024, 2022, 2020, 2018, 2016, 2014, 2012, 2010] as ano}
							<option value={ano.toString()}>{ano}</option>
						{/each}
					</select>
				</div>

				<!-- Checkbox Apenas com Gastos CEAP -->
				<div class="flex items-end">
					<label class="flex items-center gap-2.5 p-2 bg-slate-950/70 rounded-lg border border-slate-800 w-full hover:border-slate-700 transition-colors cursor-pointer select-none">
						<input
							type="checkbox"
							bind:checked={apenasComGastos}
							on:change={() => carregarPoliticos(true)}
							class="rounded bg-slate-900 border-slate-700 text-emerald-500 focus:ring-emerald-500 w-4 h-4 cursor-pointer"
						/>
						<span class="text-slate-300 font-medium truncate">Apenas gastos CEAP</span>
					</label>
				</div>
			</div>

			<!-- Tags de Filtros Ativos e Limpar -->
			{#if termoBusca || partidoSelecionado || ufSelecionada || cargoSelecionado || anoSelecionado || apenasComGastos}
				<div class="pt-3 border-t border-slate-800/80 flex flex-wrap items-center justify-between gap-2 text-xs">
					<div class="flex flex-wrap items-center gap-1.5">
						<span class="text-slate-500 font-medium mr-1">Filtros ativos:</span>
						{#if termoBusca}
							<span class="px-2 py-0.5 rounded-md bg-emerald-500/10 border border-emerald-500/30 text-emerald-300 flex items-center gap-1">
								Termo: "{termoBusca}"
								<button type="button" on:click={() => { termoBusca = ''; carregarPoliticos(true); }} class="hover:text-white">✕</button>
							</span>
						{/if}
						{#if partidoSelecionado}
							<span class="px-2 py-0.5 rounded-md bg-blue-500/10 border border-blue-500/30 text-blue-300 flex items-center gap-1">
								Partido: {partidoSelecionado}
								<button type="button" on:click={() => { partidoSelecionado = ''; carregarPoliticos(true); }} class="hover:text-white">✕</button>
							</span>
						{/if}
						{#if ufSelecionada}
							<span class="px-2 py-0.5 rounded-md bg-purple-500/10 border border-purple-500/30 text-purple-300 flex items-center gap-1">
								UF: {ufSelecionada}
								<button type="button" on:click={() => { ufSelecionada = ''; carregarPoliticos(true); }} class="hover:text-white">✕</button>
							</span>
						{/if}
						{#if cargoSelecionado}
							<span class="px-2 py-0.5 rounded-md bg-amber-500/10 border border-amber-500/30 text-amber-300 flex items-center gap-1">
								Cargo: {cargoSelecionado}
								<button type="button" on:click={() => { cargoSelecionado = ''; carregarPoliticos(true); }} class="hover:text-white">✕</button>
							</span>
						{/if}
						{#if anoSelecionado}
							<span class="px-2 py-0.5 rounded-md bg-teal-500/10 border border-teal-500/30 text-teal-300 flex items-center gap-1">
								Ano: {anoSelecionado}
								<button type="button" on:click={() => { anoSelecionado = ''; carregarPoliticos(true); }} class="hover:text-white">✕</button>
							</span>
						{/if}
						{#if apenasComGastos}
							<span class="px-2 py-0.5 rounded-md bg-slate-800 border border-slate-700 text-slate-300 flex items-center gap-1">
								Com gastos CEAP
								<button type="button" on:click={() => { apenasComGastos = false; carregarPoliticos(true); }} class="hover:text-white">✕</button>
							</span>
						{/if}
					</div>

					<button
						type="button"
						on:click={limparFiltros}
						class="text-slate-400 hover:text-rose-400 transition-colors font-medium underline flex items-center gap-1"
					>
						<span>Limpar todos os filtros</span>
					</button>
				</div>
			{/if}
		</form>
	</section>

	<!-- Feedback de Carregamento e Erros -->
	{#if loading}
		<div class="py-20 flex flex-col justify-center items-center gap-3">
			<div class="w-10 h-10 border-3 border-emerald-500 border-t-transparent rounded-full animate-spin"></div>
			<p class="text-sm text-slate-300 font-medium">Buscando parlamentares e agregando despesas...</p>
			<p class="text-xs text-slate-500">Consultando bases da Câmara dos Deputados e TSE</p>
		</div>
	{:else if erro}
		<!-- Alerta visual explícito de erro na busca -->
		<div class="p-6 bg-rose-500/10 border-2 border-rose-500/40 rounded-2xl text-rose-300 text-sm max-w-xl mx-auto space-y-3 shadow-xl">
			<div class="flex items-center gap-2.5">
				<span class="text-xl">⚠️</span>
				<h3 class="text-base font-bold text-rose-200">Erro na Consulta de Parlamentares</h3>
			</div>
			<p class="text-xs text-rose-300 leading-relaxed bg-rose-950/40 p-3 rounded-lg border border-rose-800/40 font-mono">
				{erro}
			</p>
			<div class="flex items-center gap-2 pt-1">
				<button
					type="button"
					on:click={() => carregarPoliticos(false)}
					class="px-4 py-2 bg-rose-600 hover:bg-rose-500 text-xs text-white rounded-lg transition-colors font-semibold shadow-sm"
				>
					Tentar Novamente
				</button>
				<button
					type="button"
					on:click={limparFiltros}
					class="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-xs text-slate-200 rounded-lg transition-colors font-medium"
				>
					Limpar Filtros
				</button>
			</div>
		</div>
	{:else if dados.politicos.length === 0}
		<!-- Estado Vazio -->
		<div class="p-12 text-center bg-slate-900/60 border border-slate-800 rounded-2xl max-w-lg mx-auto space-y-3">
			<div class="w-12 h-12 rounded-full bg-slate-800 text-slate-400 mx-auto flex items-center justify-center text-xl">
				🔍
			</div>
			<h3 class="text-base font-semibold text-white">Nenhum parlamentar encontrado</h3>
			<p class="text-xs text-slate-400">
				Nenhum registro corresponde aos filtros e termos selecionados. Experimente remover alguns filtros ou buscar por outro nome.
			</p>
			<div class="pt-2">
				<button
					type="button"
					on:click={limparFiltros}
					class="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-xs font-semibold transition-colors"
				>
					Redefinir Filtros
				</button>
			</div>
		</div>
	{:else}
		<!-- Grade ou Tabulação de Parlamentares -->
		<div class="space-y-6">
			<!-- Barra de Informações de Contagem e Alternador de Visualização -->
			<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 text-xs text-slate-400 px-1">
				<span>
					Mostrando <strong>{(paginaAtual - 1) * limitePorPagina + 1}</strong> a <strong>{Math.min(paginaAtual * limitePorPagina, dados.total)}</strong> de <strong>{dados.total}</strong> parlamentares
				</span>

				<div class="flex items-center gap-3">
					<!-- Alternador de Visualização: Cards vs Tabulação -->
					<div class="flex items-center bg-slate-950 border border-slate-800 p-0.5 rounded-xl shadow-inner">
						<button
							type="button"
							on:click={() => (modoVisualizacao = 'cards')}
							class="px-2.5 py-1 rounded-lg text-xs font-semibold flex items-center gap-1.5 transition-colors {modoVisualizacao === 'cards' ? 'bg-emerald-600 text-white shadow-sm' : 'text-slate-400 hover:text-slate-200 hover:bg-slate-900'}"
							title="Visualização em Cards"
						>
							<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
							</svg>
							<span>Cards</span>
						</button>
						<button
							type="button"
							on:click={() => (modoVisualizacao = 'tabela')}
							class="px-2.5 py-1 rounded-lg text-xs font-semibold flex items-center gap-1.5 transition-colors {modoVisualizacao === 'tabela' ? 'bg-emerald-600 text-white shadow-sm' : 'text-slate-400 hover:text-slate-200 hover:bg-slate-900'}"
							title="Visualização em Tabulação / Tabela"
						>
							<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 10h16M4 14h16M4 18h16" />
							</svg>
							<span>Tabulação</span>
						</button>
						<button
							type="button"
							on:click={mudarParaModoDuplicados}
							class="px-2.5 py-1 rounded-lg text-xs font-semibold flex items-center gap-1.5 transition-colors {modoVisualizacao === 'duplicados' ? 'bg-amber-600 text-white shadow-sm' : 'text-slate-400 hover:text-amber-300 hover:bg-slate-900'}"
							title="Auditoria e diagnóstico de cadastros repetidos de políticos"
						>
							<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
							</svg>
							<span>Auditoria de Duplicados</span>
							{#if resumoDuplicados && resumoDuplicados.total_grupos > 0}
								<span class="px-1.5 py-0.5 rounded-full text-[10px] font-mono font-bold {modoVisualizacao === 'duplicados' ? 'bg-amber-800 text-white' : 'bg-amber-500/20 text-amber-300 border border-amber-500/30'}">
									{resumoDuplicados.total_grupos}
								</span>
							{/if}
						</button>
					</div>

					<span>Página <strong>{paginaAtual}</strong> de <strong>{dados.total_paginas}</strong></span>
				</div>
			</div>

			{#if modoVisualizacao === 'cards'}
				<!-- Grade Responsiva de Cards de Parlamentares -->
				<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
					{#each dados.politicos as politico (politico.id)}
						<a
							href="/politicos/{politico.id}"
							on:click={(e) => {
								if (e.ctrlKey || e.metaKey || e.button === 1) return;
								e.preventDefault();
								navegarParaPolitico(politico.id);
							}}
							class="group bg-slate-900/90 hover:bg-slate-850 border border-slate-800 hover:border-emerald-500/40 rounded-2xl p-5 transition-all duration-200 hover:shadow-xl hover:shadow-emerald-950/20 flex flex-col justify-between relative overflow-hidden cursor-pointer"
						>
							<!-- Faixa sutil no topo com cor do partido -->
							<div class="absolute top-0 left-0 right-0 h-1 bg-gradient-to-r {getPartidoColor(politico.sigla_partido)}"></div>

							<div class="space-y-4">
								<!-- Header do Card: Avatar / Foto + Badges de Partido e UF -->
								<div class="flex items-start gap-3.5">
									<div class="relative w-14 h-14 min-w-[3.5rem] min-h-[3.5rem] max-w-[3.5rem] max-h-[3.5rem] flex-shrink-0">
										{#if politico.foto_base64}
											<img
												src={`data:${politico.foto_mime || 'image/jpeg'};base64,${politico.foto_base64}`}
												alt={politico.nome_urna}
												class="w-full h-full rounded-xl object-cover object-top border border-slate-700 bg-slate-950 shadow-sm"
											/>
										{:else if politico.foto_url}
											<img
												src={politico.foto_url}
												alt={politico.nome_urna}
												class="w-full h-full rounded-xl object-cover object-top border border-slate-700 bg-slate-950 shadow-sm"
												loading="lazy"
												on:error={() => {
													politico.foto_url = null;
												}}
											/>
										{:else}
											<div class="w-full h-full rounded-xl bg-gradient-to-br {getPartidoColor(politico.sigla_partido)} flex items-center justify-center font-bold text-base text-white shadow-inner border border-slate-700/60">
												{getIniciais(politico.nome_urna || politico.nome_completo)}
											</div>
										{/if}

										<!-- Botão de buscar foto oficial com 1 clique se não tiver foto salva -->
										{#if !politico.foto_base64 && !politico.foto_url}
											<button
												type="button"
												on:click={(e) => buscarFotoCard(politico, e)}
												disabled={buscandoFotosCards[politico.id]}
												class="absolute -bottom-1 -right-1 w-5 h-5 rounded-full bg-slate-800 hover:bg-emerald-600 border border-slate-600 text-[10px] flex items-center justify-center text-slate-200 hover:text-white transition-colors shadow-sm"
												title="Buscar foto oficial deste parlamentar nas bases públicas"
											>
												{#if buscandoFotosCards[politico.id]}
													<span class="inline-block w-2.5 h-2.5 border border-white border-t-transparent rounded-full animate-spin"></span>
												{:else}
													<span>📸</span>
												{/if}
											</button>
										{/if}
									</div>

									<div class="flex-1 min-w-0">
										<div class="flex items-center gap-1.5 mb-1">
											<span class="px-2 py-0.5 rounded text-[11px] font-bold bg-slate-800 border border-slate-700 text-white font-mono">
												{politico.sigla_partido || 'S/P'}
											</span>
											<span class="px-1.5 py-0.5 rounded text-[11px] font-bold bg-slate-800/80 border border-slate-700/60 text-slate-300 font-mono">
												{politico.uf || 'BR'}
											</span>
											{#if politico.tem_alertas}
												<span class="px-1.5 py-0.5 rounded text-[10px] font-bold bg-rose-500/20 text-rose-300 border border-rose-500/40 ml-auto" title="Anomalias identificadas">
													⚠️ Alerta
												</span>
											{/if}
										</div>

										<h3 class="text-sm font-bold text-white group-hover:text-emerald-400 transition-colors leading-snug truncate" title={politico.nome_urna}>
											{politico.nome_urna}
										</h3>
										<p class="text-[11px] text-slate-400 truncate mt-0.5" title={politico.nome_completo}>
											{politico.nome_completo}
										</p>
									</div>
								</div>

								<!-- Cargo, Ano Eleitoral e Mandatos do Político -->
								<div class="space-y-1.5">
									<div class="flex items-center gap-1.5 text-xs text-slate-400">
										<svg class="w-3.5 h-3.5 text-slate-500 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
											<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 13.255A23.931 23.931 0 0112 15c-3.183 0-6.22-.62-9-1.745M16 6V4a2 2 0 00-2-2h-4a2 2 0 00-2 2v2m4 6h.01M5 20h14a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
										</svg>
										<span class="font-medium text-slate-300 truncate">{politico.cargo || 'Político / Candidato'}</span>
										{#if politico.ano_eleicao}
											<span class="px-1.5 py-0.5 rounded bg-slate-800 text-emerald-400 border border-slate-700 font-mono text-[10px] font-semibold">
												{politico.ano_eleicao}
											</span>
										{/if}
										{#if politico.municipio}
											<span class="text-slate-600">•</span>
											<span class="truncate text-slate-400">{politico.municipio}</span>
										{/if}
									</div>

									<!-- Badges com todos os Mandatos / Disputas Históricas -->
									{#if politico.mandatos && politico.mandatos.length > 0}
										<div class="flex flex-wrap gap-1 pt-1">
											{#each politico.mandatos as mandato}
												{@const destacado = (cargoSelecionado && mandato.toUpperCase().includes(cargoSelecionado.toUpperCase())) || (anoSelecionado && mandato.includes(anoSelecionado))}
												<span class="px-1.5 py-0.5 rounded text-[10px] {destacado ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 font-bold' : 'bg-slate-950/80 text-slate-400 border border-slate-800'}">
													{mandato}
												</span>
											{/each}
										</div>
									{/if}
								</div>

								<!-- Painel Financeiro CEAP -->
								<div class="bg-slate-950/70 border border-slate-800/80 rounded-xl p-3 space-y-1.5 shadow-inner">
									<div class="flex items-center justify-between text-xs">
										<span class="text-slate-400">Total Cota CEAP:</span>
										<span class="font-bold text-emerald-400 font-mono">
											{formatarMoeda(politico.total_despesas_ceap)}
										</span>
									</div>
									<div class="flex items-center justify-between text-[11px] text-slate-500">
										<span>Notas Declaradas:</span>
										<span class="font-mono text-slate-300 font-semibold">
											{politico.total_itens_ceap} {politico.total_itens_ceap === 1 ? 'nota' : 'notas'}
										</span>
									</div>
									{#if politico.total_bens_declarados > 0}
										<div class="flex items-center justify-between text-[11px] text-slate-500 pt-1 border-t border-slate-900">
											<span>Bens Declarados:</span>
											<span class="font-mono text-slate-400">
												{formatarMoeda(politico.total_bens_declarados)}
											</span>
										</div>
									{/if}
								</div>
							</div>

							<!-- Rodapé do Card com CTA -->
							<div class="mt-4 pt-3 border-t border-slate-800/60 flex items-center justify-between text-xs text-slate-400 group-hover:text-emerald-400 transition-colors">
								<span class="font-medium text-[11px]">Ver perfil & mapa</span>
								<div class="flex items-center gap-1">
									<span class="text-[10px] opacity-0 group-hover:opacity-100 transition-opacity font-semibold">Acessar</span>
									<svg class="w-4 h-4 transform group-hover:translate-x-1 transition-transform" fill="none" viewBox="0 0 24 24" stroke="currentColor">
										<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
									</svg>
								</div>
							</div>
						</a>
					{/each}
				</div>
			{:else if modoVisualizacao === 'tabela'}
				<!-- Tabulação Detalhada de Parlamentares (Visualização em Tabela) -->
				<div class="overflow-x-auto rounded-2xl border border-slate-800 bg-slate-900/90 shadow-xl">
					<table class="w-full text-left text-xs text-slate-300">
						<thead class="bg-slate-950/80 text-[11px] uppercase font-bold text-slate-400 border-b border-slate-800 tracking-wider">
							<tr>
								<th class="py-3 px-4 w-12 text-center">Foto</th>
								<th class="py-3 px-4 min-w-[200px]">Parlamentar / Nome</th>
								<th class="py-3 px-3 w-28 text-center">Partido / UF</th>
								<th class="py-3 px-4 min-w-[150px]">Cargo & Eleição</th>
								<th class="py-3 px-4 min-w-[220px]">Tabulação de Mandatos</th>
								<th class="py-3 px-4 text-right min-w-[120px]">Cota CEAP</th>
								<th class="py-3 px-4 text-right min-w-[120px]">Bens Declarados</th>
								<th class="py-3 px-3 text-center w-24">Status</th>
								<th class="py-3 px-4 text-center w-28">Ação</th>
							</tr>
						</thead>
						<tbody class="divide-y divide-slate-800/60">
							{#each dados.politicos as politico (politico.id)}
								<tr class="hover:bg-slate-800/50 transition-colors">
									<!-- Foto miniatura -->
									<td class="py-3 px-4 text-center">
										<div class="relative w-9 h-9 min-w-[2.25rem] min-h-[2.25rem] mx-auto">
											{#if politico.foto_base64}
												<img
													src={`data:${politico.foto_mime || 'image/jpeg'};base64,${politico.foto_base64}`}
													alt={politico.nome_urna}
													class="w-full h-full rounded-lg object-cover object-top border border-slate-700 bg-slate-950 shadow-sm"
												/>
											{:else if politico.foto_url}
												<img
													src={politico.foto_url}
													alt={politico.nome_urna}
													class="w-full h-full rounded-lg object-cover object-top border border-slate-700 bg-slate-950 shadow-sm"
													loading="lazy"
													on:error={() => {
														politico.foto_url = null;
													}}
												/>
											{:else}
												<div class="w-full h-full rounded-lg bg-gradient-to-br {getPartidoColor(politico.sigla_partido)} flex items-center justify-center font-bold text-xs text-white border border-slate-700/60">
													{getIniciais(politico.nome_urna || politico.nome_completo)}
												</div>
											{/if}

											{#if !politico.foto_base64 && !politico.foto_url}
												<button
													type="button"
													on:click={(e) => buscarFotoCard(politico, e)}
													disabled={buscandoFotosCards[politico.id]}
													class="absolute -bottom-1 -right-1 w-4 h-4 rounded-full bg-slate-800 hover:bg-emerald-600 border border-slate-600 text-[9px] flex items-center justify-center text-slate-200 transition-colors shadow-sm"
													title="Buscar foto oficial"
												>
													{#if buscandoFotosCards[politico.id]}
														<span class="inline-block w-2 h-2 border border-white border-t-transparent rounded-full animate-spin"></span>
													{:else}
														<span>📸</span>
													{/if}
												</button>
											{/if}
										</div>
									</td>

									<!-- Nome Urna & Nome Completo -->
									<td class="py-3 px-4">
										<button
											type="button"
											on:click={() => navegarParaPolitico(politico.id)}
											class="text-left font-bold text-white hover:text-emerald-400 transition-colors block text-sm"
										>
											{politico.nome_urna}
										</button>
										<div class="text-[11px] text-slate-400 truncate max-w-xs mt-0.5" title={politico.nome_completo}>
											{politico.nome_completo}
										</div>
										{#if politico.cpf_mascarado && politico.cpf_mascarado !== '-4'}
											<div class="text-[10px] text-slate-500 font-mono">
												CPF: {politico.cpf_mascarado}
											</div>
										{/if}
									</td>

									<!-- Partido & UF -->
									<td class="py-3 px-3 text-center">
										<div class="inline-flex items-center gap-1">
											<span class="px-2 py-0.5 rounded text-[11px] font-bold bg-slate-800 border border-slate-700 text-white font-mono">
												{politico.sigla_partido || 'S/P'}
											</span>
											<span class="px-1.5 py-0.5 rounded text-[11px] font-bold bg-slate-800/80 border border-slate-700/60 text-slate-300 font-mono">
												{politico.uf || 'BR'}
											</span>
										</div>
									</td>

									<!-- Cargo & Eleição -->
									<td class="py-3 px-4">
										<div class="font-medium text-slate-200">
											{politico.cargo || 'Político / Candidato'}
										</div>
										<div class="flex items-center gap-1.5 text-[11px] text-slate-400 mt-0.5">
											{#if politico.ano_eleicao}
												<span class="px-1.5 py-0.2 rounded bg-slate-800 text-emerald-400 border border-slate-700 font-mono font-semibold">
													{politico.ano_eleicao}
												</span>
											{/if}
											{#if politico.municipio}
												<span>{politico.municipio}</span>
											{/if}
										</div>
									</td>

									<!-- Tabulação de Mandatos -->
									<td class="py-3 px-4">
										{#if politico.mandatos && politico.mandatos.length > 0}
											<div class="flex flex-wrap gap-1 max-w-sm">
												{#each politico.mandatos as mandato}
													{@const destacado = (cargoSelecionado && mandato.toUpperCase().includes(cargoSelecionado.toUpperCase())) || (anoSelecionado && mandato.includes(anoSelecionado))}
													<span class="px-1.5 py-0.5 rounded text-[10px] {destacado ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 font-bold' : 'bg-slate-950/80 text-slate-400 border border-slate-800'}">
														{mandato}
													</span>
												{/each}
											</div>
										{:else}
											<span class="text-slate-500 text-[11px] italic">Sem histórico adicional</span>
										{/if}
									</td>

									<!-- Cota CEAP -->
									<td class="py-3 px-4 text-right">
										<div class="font-mono font-bold {politico.total_despesas_ceap > 0 ? 'text-emerald-400' : 'text-slate-500'}">
											{formatarMoeda(politico.total_despesas_ceap)}
										</div>
										<div class="text-[10px] text-slate-500">
											{politico.total_itens_ceap} {politico.total_itens_ceap === 1 ? 'nota' : 'notas'}
										</div>
									</td>

									<!-- Bens Declarados -->
									<td class="py-3 px-4 text-right">
										{#if politico.total_bens_declarados > 0}
											<div class="font-mono text-slate-300 font-semibold">
												{formatarMoeda(politico.total_bens_declarados)}
											</div>
										{:else}
											<span class="text-slate-600 font-mono">-</span>
										{/if}
									</td>

									<!-- Status / Alertas -->
									<td class="py-3 px-3 text-center">
										{#if politico.tem_alertas}
											<span class="px-1.5 py-0.5 rounded text-[10px] font-bold bg-rose-500/20 text-rose-300 border border-rose-500/40 whitespace-nowrap">
												⚠️ Alerta
											</span>
										{:else}
											<span class="px-1.5 py-0.5 rounded text-[10px] bg-slate-800 text-slate-400 border border-slate-700 whitespace-nowrap">
												Regular
											</span>
										{/if}
									</td>

									<!-- Ações -->
									<td class="py-3 px-4 text-center">
										<button
											type="button"
											on:click={() => navegarParaPolitico(politico.id)}
											class="px-2.5 py-1.5 rounded-lg bg-emerald-600/20 hover:bg-emerald-600 text-emerald-300 hover:text-white border border-emerald-500/40 text-[11px] font-semibold transition-colors flex items-center justify-center gap-1 mx-auto"
										>
											<span>Perfil</span>
											<svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
												<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
											</svg>
										</button>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{:else if modoVisualizacao === 'duplicados'}
				<!-- Painel de Auditoria de Cadastros Repetidos & Divergências de Ingestão -->
				<div class="space-y-6">
					<!-- Banner Analítico de Diagnóstico -->
					<div class="bg-gradient-to-r from-amber-950/40 via-slate-900 to-slate-900 border border-amber-500/30 rounded-2xl p-6 shadow-xl space-y-4">
						<div class="flex flex-col md:flex-row md:items-center justify-between gap-4">
							<div class="flex items-start gap-3">
								<div class="w-10 h-10 rounded-xl bg-amber-500/10 border border-amber-500/30 text-amber-400 flex items-center justify-center text-xl flex-shrink-0">
									⚖️
								</div>
								<div>
									<h2 class="text-base sm:text-lg font-bold text-white flex items-center gap-2">
										Diagnóstico de Cadastros Repetidos
										<span class="px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-300 border border-amber-500/40 text-[11px] font-mono">
											Auditoria Cadastral
										</span>
									</h2>
									<p class="text-xs text-slate-300 mt-1 max-w-2xl leading-relaxed">
										Identificação analítica de pessoas físicas com múltiplos registros na base devido à rotação de identificadores oficiais e falta de chave primária natural consistente.
									</p>
								</div>
							</div>

							<!-- Botão de Ação: Mesclar Automático com 100% de Certeza -->
							<button
								type="button"
								on:click={unificarAutomaticoTodos}
								disabled={mesclandoAutomatico}
								class="px-4 py-2.5 bg-amber-600 hover:bg-amber-500 text-white rounded-xl text-xs font-bold transition-all shadow-lg hover:shadow-amber-950/40 disabled:opacity-50 flex items-center gap-2 flex-shrink-0"
							>
								{#if mesclandoAutomatico}
									<div class="w-3.5 h-3.5 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
									<span>Unificando grupos...</span>
								{:else}
									<span>⚡ Unificar Grupos Confirmados</span>
								{/if}
							</button>
						</div>

						<!-- Métricas do Diagnóstico -->
						<div class="grid grid-cols-1 sm:grid-cols-3 gap-3 pt-2">
							<div class="bg-slate-950/80 border border-slate-800 rounded-xl p-3.5">
								<div class="text-[11px] text-slate-400 font-medium">Grupos Repetidos Identificados</div>
								<div class="text-xl font-extrabold text-amber-400 font-mono mt-0.5">
									{resumoDuplicados?.total_grupos || 0}
								</div>
								<div class="text-[10px] text-slate-500 mt-0.5">candidatos com múltiplos registros</div>
							</div>

							<div class="bg-slate-950/80 border border-slate-800 rounded-xl p-3.5">
								<div class="text-[11px] text-slate-400 font-medium">Cadastros Redundantes Estimados</div>
								<div class="text-xl font-extrabold text-rose-400 font-mono mt-0.5">
									{resumoDuplicados?.total_registros_duplicados || 0}
								</div>
								<div class="text-[10px] text-slate-500 mt-0.5">linhas excedentes que podem ser consolidadas</div>
							</div>

							<div class="bg-slate-950/80 border border-slate-800 rounded-xl p-3.5">
								<div class="text-[11px] text-slate-400 font-medium">Confirmação Máxima (100% Certeza)</div>
								<div class="text-xl font-extrabold text-emerald-400 font-mono mt-0.5">
									{resumoDuplicados?.grupos_nascimento_exato || 0}
								</div>
								<div class="text-[10px] text-slate-500 mt-0.5">mesmo nome civil completo e data de nascimento</div>
							</div>
						</div>

						<!-- Análise Técnica da Causa Raiz -->
						<div class="bg-slate-950/60 border border-slate-800/80 rounded-xl p-3.5 text-xs text-slate-300 space-y-1.5 leading-relaxed">
							<div class="font-bold text-white flex items-center gap-1.5 text-[11px]">
								<span>📌</span> Por que existem cadastros repetidos na base?
							</div>
							<p class="text-[11px] text-slate-400">
								<strong>1. Identificador de Pleito do TSE:</strong> O TSE gera um novo código sequencial (<code>sq_candidato</code>) a cada nova eleição ou substituição de candidatura. Se a ingestão tratar o sequencial como identificador de pessoa física, o mesmo político é recriado como um novo registro.
							</p>
							<p class="text-[11px] text-slate-400">
								<strong>2. Divergência CEAP x TSE:</strong> Gastos parlamentares da Câmara trazem o nome parlamentar (ex: "Abilio Brunini"), enquanto o TSE registra o nome civil (ex: "ABILIO JACQUES BRUNINI MOUMER"), provocando duplicação quando não há conciliação prévia.
							</p>
							<p class="text-[11px] text-slate-400">
								<strong>3. Mascaramento LGPD:</strong> Nas bases recentes de 2022/2024, o TSE oculta o CPF público colocando <code>-4</code>, impedindo validação documental unívoca simples e exigindo cruzamento determinístico por data de nascimento e nome completo.
							</p>
						</div>
					</div>

					<!-- Feedback de Ação -->
					{#if msgAuditoria}
						<div class="p-3 bg-emerald-500/10 border border-emerald-500/30 rounded-xl text-emerald-300 text-xs flex items-center justify-between">
							<span>✅ {msgAuditoria}</span>
							<button type="button" on:click={() => (msgAuditoria = null)} class="hover:text-white font-bold">✕</button>
						</div>
					{/if}

					<!-- Lista de Grupos Duplicados -->
					{#if carregandoDuplicados}
						<div class="py-16 flex flex-col justify-center items-center gap-3">
							<div class="w-8 h-8 border-3 border-amber-500 border-t-transparent rounded-full animate-spin"></div>
							<p class="text-xs text-slate-400">Varrendo base e agrupando candidatos repetidos...</p>
						</div>
					{:else if !relatorioDuplicados || relatorioDuplicados.grupos.length === 0}
						<div class="p-10 text-center bg-slate-900/60 border border-slate-800 rounded-2xl max-w-lg mx-auto space-y-2">
							<div class="text-2xl">🎉</div>
							<h3 class="text-sm font-bold text-white">Nenhum cadastro duplicado pendente!</h3>
							<p class="text-xs text-slate-400">
								Todos os registros analisados estão unificados e consistentes.
							</p>
						</div>
					{:else}
						<div class="space-y-4">
							{#each relatorioDuplicados.grupos as grupo (grupo.id_grupo)}
								<div class="bg-slate-900/90 border border-amber-500/20 hover:border-amber-500/40 rounded-2xl p-5 shadow-lg space-y-4 transition-colors">
									<!-- Cabeçalho do Grupo -->
									<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 pb-3 border-b border-slate-800">
										<div class="flex items-center gap-2">
											<span class="w-2.5 h-2.5 rounded-full bg-amber-400"></span>
											<h3 class="font-bold text-white text-sm">
												{grupo.politicos[0].nome_completo}
											</h3>
											{#if grupo.politicos[0].data_nascimento}
												<span class="text-xs text-slate-400 font-mono">
													(Nasc: {grupo.politicos[0].data_nascimento})
												</span>
											{/if}
										</div>

										<div class="flex items-center gap-2">
											<span class="px-2 py-0.5 rounded text-[10px] font-bold bg-amber-500/20 text-amber-300 border border-amber-500/30">
												{grupo.confianca}
											</span>
											<span class="text-[11px] text-slate-400">
												{grupo.politicos.length} registros repetidos
											</span>
										</div>
									</div>

									<p class="text-[11px] text-slate-400 italic">
										ℹ️ {grupo.motivo}
									</p>

									<!-- Grade Comparativa dos Registros Repetidos -->
									<div class="grid grid-cols-1 md:grid-cols-2 gap-3">
										{#each grupo.politicos as p (p.id)}
											{@const isCanonico = p.id === grupo.sugestao_canonico_id}
											<div class="p-4 rounded-xl border {isCanonico ? 'bg-emerald-950/20 border-emerald-500/40 ring-1 ring-emerald-500/30' : 'bg-slate-950/80 border-slate-800'} space-y-3 relative">
												<div class="flex items-start justify-between gap-2">
													<div class="flex items-center gap-2.5">
														<div class="w-10 h-10 rounded-lg overflow-hidden flex-shrink-0 bg-slate-900 border border-slate-800">
															{#if p.foto_base64}
																<img
																	src={`data:${p.foto_mime || 'image/jpeg'};base64,${p.foto_base64}`}
																	alt={p.nome_urna}
																	class="w-full h-full object-cover object-top"
																/>
															{:else if p.foto_url}
																<img
																	src={p.foto_url}
																	alt={p.nome_urna}
																	class="w-full h-full object-cover object-top"
																/>
															{:else}
																<div class="w-full h-full flex items-center justify-center font-bold text-xs text-white bg-slate-800">
																	{getIniciais(p.nome_urna || p.nome_completo)}
																</div>
															{/if}
														</div>

														<div class="min-w-0">
															<div class="flex items-center gap-1.5">
																<span class="text-xs font-bold text-white truncate" title={p.nome_urna}>
																	{p.nome_urna}
																</span>
																<span class="text-[10px] text-slate-500 font-mono">#{p.id}</span>
															</div>
															<div class="text-[11px] text-slate-400 truncate" title={p.nome_completo}>
																{p.nome_completo}
															</div>
														</div>
													</div>

													{#if isCanonico}
														<span class="px-2 py-0.5 rounded text-[10px] font-bold bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 whitespace-nowrap">
															⭐ Registro Principal
														</span>
													{/if}
												</div>

												<!-- Metadados -->
												<div class="space-y-1 text-[11px] text-slate-300 pt-1 border-t border-slate-800/80">
													<div class="flex justify-between">
														<span class="text-slate-500">Partido / UF:</span>
														<span class="font-bold text-slate-200">{p.sigla_partido} - {p.uf}</span>
													</div>
													<div class="flex justify-between">
														<span class="text-slate-500">Cargo:</span>
														<span class="text-slate-200">{p.cargo} {#if p.ano_eleicao}({p.ano_eleicao}){/if}</span>
													</div>
													{#if p.sq_candidato}
														<div class="flex justify-between font-mono text-[10px]">
															<span class="text-slate-500">SQ TSE:</span>
															<span class="text-slate-400">{p.sq_candidato}</span>
														</div>
													{/if}
													{#if p.total_despesas_ceap > 0}
														<div class="flex justify-between">
															<span class="text-slate-500">Gastos CEAP:</span>
															<span class="text-emerald-400 font-mono font-bold">{formatarMoeda(p.total_despesas_ceap)}</span>
														</div>
													{/if}
												</div>

												<!-- Mandatos Tabulados -->
												{#if p.mandatos && p.mandatos.length > 0}
													<div class="pt-1">
														<div class="text-[10px] text-slate-500 mb-1">Mandatos neste ID:</div>
														<div class="flex flex-wrap gap-1">
															{#each p.mandatos as m}
																<span class="px-1.5 py-0.5 rounded text-[10px] bg-slate-900 border border-slate-800 text-slate-300">
																	{m}
																</span>
															{/each}
														</div>
													</div>
												{/if}

												<!-- Botões de Ação -->
												<div class="pt-2 flex items-center justify-between gap-2 border-t border-slate-800/80">
													<button
														type="button"
														on:click={() => navegarParaPolitico(p.id)}
														class="text-[11px] text-slate-400 hover:text-white underline"
													>
														Ver Dossiê
													</button>

													{#if !isCanonico}
														<button
															type="button"
															on:click={() => mesclarRegistros(grupo.sugestao_canonico_id, p.id)}
															disabled={mesclandoIds[p.id]}
															class="px-2.5 py-1 bg-amber-600 hover:bg-amber-500 text-white rounded-lg text-xs font-bold transition-colors disabled:opacity-50 flex items-center gap-1 shadow-sm"
															title="Transfere candidaturas e bens para o registro #{grupo.sugestao_canonico_id} e remove este registro redundante"
														>
															{#if mesclandoIds[p.id]}
																<span class="w-2.5 h-2.5 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
																<span>Mesclando...</span>
															{:else}
																<span>⚡ Unificar no #{grupo.sugestao_canonico_id}</span>
															{/if}
														</button>
													{/if}
												</div>
											</div>
										{/each}
									</div>
								</div>
							{/each}

							<!-- Paginação de Duplicados -->
							{#if relatorioDuplicados && relatorioDuplicados.total_paginas > 1}
								<div class="pt-4 flex items-center justify-between text-xs text-slate-400">
									<div>
										Página <strong class="text-white">{paginaDuplicados}</strong> de <strong class="text-white">{relatorioDuplicados.total_paginas}</strong>
										({relatorioDuplicados.total_grupos} grupos identificados)
									</div>

									<div class="flex items-center gap-2">
										<button
											type="button"
											on:click={() => carregarRelatorioDuplicados(paginaDuplicados - 1)}
											disabled={paginaDuplicados <= 1}
											class="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-lg disabled:opacity-40"
										>
											Anterior
										</button>
										<button
											type="button"
											on:click={() => carregarRelatorioDuplicados(paginaDuplicados + 1)}
											disabled={paginaDuplicados >= relatorioDuplicados.total_paginas}
											class="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-lg disabled:opacity-40"
										>
											Próxima
										</button>
									</div>
								</div>
							{/if}
						</div>
					{/if}
				</div>
			{/if}

			<!-- Paginação Normal (Cards e Tabela) -->
			{#if modoVisualizacao !== 'duplicados' && dados.total_paginas > 1}
				<div class="pt-6 pb-2 border-t border-slate-800 flex flex-col sm:flex-row items-center justify-between gap-4 text-xs">
					<div class="text-slate-400">
						Página <span class="font-bold text-white">{paginaAtual}</span> de <span class="font-bold text-white">{dados.total_paginas}</span>
					</div>

					<div class="flex items-center gap-1.5">
						<!-- Botão Anterior -->
						<button
							type="button"
							on:click={() => mudarPagina(paginaAtual - 1)}
							disabled={paginaAtual <= 1}
							class="px-3 py-1.5 rounded-lg border border-slate-800 bg-slate-900 text-slate-300 hover:bg-slate-800 hover:text-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors font-medium flex items-center gap-1"
						>
							<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
							</svg>
							<span>Anterior</span>
						</button>

						<!-- Números de Página Inteligentes -->
						<div class="hidden sm:flex items-center gap-1">
							{#each Array.from({ length: Math.min(5, dados.total_paginas) }, (_, i) => {
								let p = paginaAtual - 2 + i;
								if (paginaAtual <= 3) p = i + 1;
								else if (paginaAtual >= dados.total_paginas - 2) p = dados.total_paginas - 4 + i;
								return p;
							}).filter(p => p >= 1 && p <= dados.total_paginas) as num}
								<button
									type="button"
									on:click={() => mudarPagina(num)}
									class="w-8 h-8 rounded-lg text-xs font-semibold transition-colors {num === paginaAtual ? 'bg-emerald-600 text-white font-bold shadow-md' : 'bg-slate-900 border border-slate-800 text-slate-400 hover:text-white hover:bg-slate-800'}"
								>
									{num}
								</button>
							{/each}
						</div>

						<!-- Botão Próximo -->
						<button
							type="button"
							on:click={() => mudarPagina(paginaAtual + 1)}
							disabled={paginaAtual >= dados.total_paginas}
							class="px-3 py-1.5 rounded-lg border border-slate-800 bg-slate-900 text-slate-300 hover:bg-slate-800 hover:text-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors font-medium flex items-center gap-1"
						>
							<span>Próxima</span>
							<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
							</svg>
						</button>
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>
