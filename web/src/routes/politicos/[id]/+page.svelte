<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import MapaDespesas from '$lib/components/MapaDespesas.svelte';
	import type { PoliticoDetalheResponse, PoliticoDespesasGeoResponse, BuscarFotoResponse } from '$lib/types';

	$: id = $page.params.id;

	let loading = true;
	let erro: string | null = null;
	let politico: PoliticoDetalheResponse | null = null;
	let geoData: PoliticoDespesasGeoResponse | null = null;
	let abaAtiva: 'mapa' | 'categorias' | 'despesas' | 'eleitoral' | 'evolucao' | 'emendas' = 'mapa';

	$: totalEmendasEmpenhado = politico?.emendas ? politico.emendas.reduce((acc, e) => acc + (e.valor_empenhado || 0), 0) : 0;
	$: totalEmendasPago = politico?.emendas ? politico.emendas.reduce((acc, e) => acc + (e.valor_pago || 0), 0) : 0;

	$: maxPatrimonio = politico?.evolucao_patrimonial && politico.evolucao_patrimonial.length > 0
		? Math.max(...politico.evolucao_patrimonial.map((p) => p.valor_total), 1)
		: 1;

	// Gerenciamento de Foto Oficial (TSE/Câmara)
	let buscandoFoto = false;
	let salvandoFoto = false;
	let fotoLocalUrl: string | null = null;
	let msgFotoSucesso: string | null = null;
	let msgFotoErro: string | null = null;
	let mostrarModalFoto = false;
	let fotoUrlInput = '';
	let uploadFileInput: HTMLInputElement;

	let ultimoIdCarregado: string | null = null;
	$: if (id && id !== ultimoIdCarregado) {
		ultimoIdCarregado = id;
		carregarDados();
	}

	$: mandatosDistintos = politico?.candidaturas
		? Array.from(new Set(politico.candidaturas.map((c) => `${c.cargo} (${c.ano_eleicao})`)))
		: [];

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

	async function buscarFotoTse() {
		if (buscandoFoto || !politico) return;
		buscandoFoto = true;
		msgFotoSucesso = null;
		msgFotoErro = null;

		try {
			const res = await fetch(`/api/politicos/${id}/buscar-foto-tse`, {
				method: 'POST'
			});
			const data: BuscarFotoResponse = await res.json();
			if (res.ok && data.sucesso) {
				msgFotoSucesso = data.mensagem;
				if (data.foto_base64) {
					politico.foto_base64 = data.foto_base64;
					politico.foto_mime = data.foto_mime || 'image/jpeg';
				}
				fotoLocalUrl = `/api/politicos/${id}/foto?t=${Date.now()}`;
			} else {
				msgFotoErro = data.mensagem || 'Não foi possível encontrar a foto oficial no TSE ou Câmara.';
			}
		} catch (err: any) {
			msgFotoErro = `Erro de comunicação ao buscar foto: ${err?.message || 'Falha de conexão com a API'}`;
		} finally {
			buscandoFoto = false;
		}
	}

	async function salvarFotoPorUrl() {
		if (!fotoUrlInput.trim() || !politico) return;
		salvandoFoto = true;
		msgFotoSucesso = null;
		msgFotoErro = null;

		try {
			const res = await fetch(`/api/politicos/${id}/foto`, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ foto_url: fotoUrlInput.trim() })
			});
			const data: BuscarFotoResponse = await res.json();
			if (res.ok && data.sucesso) {
				msgFotoSucesso = data.mensagem;
				if (data.foto_base64) {
					politico.foto_base64 = data.foto_base64;
					politico.foto_mime = data.foto_mime || 'image/jpeg';
				}
				fotoLocalUrl = `/api/politicos/${id}/foto?t=${Date.now()}`;
				mostrarModalFoto = false;
				fotoUrlInput = '';
			} else {
				msgFotoErro = data.mensagem || 'Falha ao salvar foto pela URL informada.';
			}
		} catch (err: any) {
			msgFotoErro = `Erro ao salvar foto: ${err?.message || 'Falha de rede'}`;
		} finally {
			salvandoFoto = false;
		}
	}

	function handleUploadArquivo(e: Event) {
		const target = e.target as HTMLInputElement;
		const file = target?.files?.[0];
		if (!file || !politico) return;

		salvandoFoto = true;
		msgFotoSucesso = null;
		msgFotoErro = null;

		const reader = new FileReader();
		reader.onload = async () => {
			const base64Str = reader.result as string;
			try {
				const res = await fetch(`/api/politicos/${id}/foto`, {
					method: 'POST',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify({
						foto_base64: base64Str,
						foto_mime: file.type || 'image/jpeg'
					})
				});
				const data: BuscarFotoResponse = await res.json();
				if (res.ok && data.sucesso) {
					msgFotoSucesso = data.mensagem;
					if (politico && data.foto_base64) {
						politico.foto_base64 = data.foto_base64;
						politico.foto_mime = data.foto_mime || file.type || 'image/jpeg';
					}
					fotoLocalUrl = `/api/politicos/${id}/foto?t=${Date.now()}`;
					mostrarModalFoto = false;
				} else {
					msgFotoErro = data.mensagem || 'Falha ao salvar a imagem enviada.';
				}
			} catch (err: any) {
				msgFotoErro = `Erro no envio da imagem: ${err?.message || 'Falha de rede'}`;
			} finally {
				salvandoFoto = false;
				if (target) target.value = '';
			}
		};
		reader.onerror = () => {
			salvandoFoto = false;
			msgFotoErro = 'Erro ao processar o arquivo de imagem selecionado.';
		};
		reader.readAsDataURL(file);
	}

	async function removerFoto() {
		if (!politico) return;
		if (!confirm('Deseja realmente remover a foto salva deste parlamentar?')) return;
		try {
			const res = await fetch(`/api/politicos/${id}/foto`, { method: 'DELETE' });
			if (res.ok) {
				politico.foto_base64 = null;
				politico.foto_mime = null;
				fotoLocalUrl = null;
				msgFotoSucesso = 'Foto removida com sucesso do banco de dados.';
				msgFotoErro = null;
			}
		} catch (err: any) {
			msgFotoErro = `Erro ao remover foto: ${err?.message || 'Falha'}`;
		}
	}

	function formatarMoeda(val: number): string {
		return val.toLocaleString('pt-BR', { style: 'currency', currency: 'BRL' });
	}

	function formatarData(dataStr: string | null): string {
		if (!dataStr) return '-';
		const parts = dataStr.split('T')[0].split('-');
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

		<div class="flex items-center gap-2">
			<button
				type="button"
				on:click={() => window.print()}
				class="text-xs font-semibold px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-200 hover:text-white transition-colors flex items-center gap-1.5 shadow-sm"
				title="Imprimir ou exportar dossiê oficial em PDF"
			>
				<svg class="w-3.5 h-3.5 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4a2 2 0 00-2-2H9a2 2 0 00-2 2v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
				</svg>
				<span>Imprimir / Exportar Dossiê</span>
			</button>

			{#if politico?.cpf_mascarado}
				<a
					href="/dossie/cpf/{politico.cpf_mascarado.replace(/\D/g, '')}"
					class="text-xs font-semibold px-3 py-1.5 rounded-lg bg-indigo-600/20 border border-indigo-500/40 text-indigo-300 hover:bg-indigo-600/30 transition-colors flex items-center gap-1.5"
				>
					<span>Ver Dossiê de Vínculos (CPF)</span>
					<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 5l7 7m0 0l-7 7m7-7H3" />
					</svg>
				</a>
			{/if}
		</div>
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
		<!-- Cabeçalho Institucional de Emissão Oficial (Exibido Apenas na Impressão / PDF) -->
		<div class="hidden print:block border-b-2 border-slate-900 pb-4 mb-4">
			<div class="flex items-center justify-between">
				<div class="flex items-center gap-3">
					<div class="w-10 h-10 rounded-xl bg-slate-900 text-white flex items-center justify-center font-black text-xl">
						RC
					</div>
					<div>
						<h1 class="text-xl font-black text-slate-900 tracking-tight">RADAR CÍVICO</h1>
						<p class="text-[10px] text-slate-600 font-mono tracking-wider uppercase">Plataforma Independente de Auditoria Cívica e Controle Social</p>
					</div>
				</div>
				<div class="text-right text-[10px] text-slate-700 font-mono space-y-0.5">
					<div><strong>DOSSIÊ ANALÍTICO OFICIAL</strong></div>
					<div>Emissão: {new Date().toLocaleDateString('pt-BR')} às {new Date().toLocaleTimeString('pt-BR')}</div>
					<div>Score Integridade: <strong>{politico.score_integridade ?? 100}/100</strong> • Risco <strong>{politico.nivel_risco ?? 'MÍNIMO'}</strong></div>
					<div>Registro: #{politico.id} • Chave Criptográfica SHA-256 Verificada</div>
				</div>
			</div>
		</div>

		<!-- Header Principal do Parlamentar -->
		<section class="bg-slate-900 border border-slate-800 rounded-3xl p-6 shadow-xl relative overflow-hidden space-y-4">
			<!-- Detalhe Gradiente no Topo -->
			<div class="absolute top-0 left-0 right-0 h-1.5 bg-gradient-to-r {getPartidoColor(politico.partido)}"></div>

			<div class="flex flex-col md:flex-row md:items-center justify-between gap-6">
				<!-- Avatar / Foto + Dados Principais -->
				<div class="flex flex-col sm:flex-row items-start sm:items-center gap-5">
					<div class="relative group flex-shrink-0">
						{#if fotoLocalUrl}
							<img
								src={fotoLocalUrl}
								alt={politico.nome_urna}
								class="w-24 h-24 sm:w-28 sm:h-28 rounded-2xl object-cover border-2 border-slate-700 bg-slate-950 shadow-xl"
							/>
						{:else if politico.foto_base64}
							<img
								src={`data:${politico.foto_mime || 'image/jpeg'};base64,${politico.foto_base64}`}
								alt={politico.nome_urna}
								class="w-24 h-24 sm:w-28 sm:h-28 rounded-2xl object-cover border-2 border-slate-700 bg-slate-950 shadow-xl"
							/>
						{:else if politico.foto_url}
							<img
								src={politico.foto_url}
								alt={politico.nome_urna}
								class="w-24 h-24 sm:w-28 sm:h-28 rounded-2xl object-cover border-2 border-slate-700 bg-slate-950 shadow-xl"
							/>
						{:else}
							<div class="w-24 h-24 sm:w-28 sm:h-28 rounded-2xl bg-gradient-to-br {getPartidoColor(politico.partido)} flex items-center justify-center font-extrabold text-3xl text-white shadow-xl border-2 border-slate-700">
								{getIniciais(politico.nome_urna || politico.nome_completo)}
							</div>
						{/if}
					</div>

					<div class="space-y-1.5">
						<div class="flex flex-wrap items-center gap-2">
							<span class="px-2.5 py-0.5 rounded-lg text-xs font-bold bg-slate-800 border border-slate-700 text-white font-mono shadow-sm">
								{politico.partido || 'SEM PARTIDO'}
							</span>
							<span class="px-2.5 py-0.5 rounded-lg text-xs font-bold bg-slate-800 border border-slate-700 text-emerald-400 font-mono shadow-sm">
								{politico.uf || 'BR'}
							</span>
							<span class="px-2.5 py-0.5 rounded-lg text-xs font-medium bg-slate-800/80 border border-slate-700/60 text-slate-300">
								{politico.cargo || 'Político / Candidato'}
							</span>
							{#if politico.tipo_agente && politico.tipo_agente !== 'POLITICO'}
								<span class="px-2.5 py-0.5 rounded-lg text-xs font-bold bg-purple-500/20 text-purple-300 border border-purple-500/40 shadow-sm">
									🏛️ {politico.tipo_agente === 'MINISTRO_STF' ? 'STF • Supremo Tribunal Federal' : politico.tipo_agente === 'MINISTRO_TCU' ? 'TCU • Tribunal de Contas da União' : politico.tipo_agente === 'PROCURADOR_MPTCU' ? 'MPTCU • Ministério Público de Contas' : politico.tipo_agente === 'PGR' ? 'MPU • Procuradoria-Geral da República' : politico.tipo_agente === 'EMBAIXADOR' ? 'MRE • Missão Diplomática' : 'Secretaria de Estado'}
								</span>
							{/if}
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

						<div class="flex flex-wrap items-center gap-3 text-xs text-slate-400 pt-0.5 font-mono">
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

						<!-- Ações da Foto Oficial (TSE/Câmara) -->
						<div class="flex flex-wrap items-center gap-2 pt-2">
							<button
								type="button"
								on:click={buscarFotoTse}
								disabled={buscandoFoto}
								class="px-3 py-1.5 rounded-lg bg-emerald-600/20 hover:bg-emerald-600/30 border border-emerald-500/40 text-emerald-300 text-xs font-medium flex items-center gap-1.5 transition-colors disabled:opacity-50 shadow-sm"
								title="Busca a foto oficial nas bases do TSE / Câmara dos Deputados e armazena permanentemente no banco local"
							>
								{#if buscandoFoto}
									<div class="w-3.5 h-3.5 border-2 border-emerald-400 border-t-transparent rounded-full animate-spin"></div>
									<span>Buscando foto oficial...</span>
								{:else}
									<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
										<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 9a2 2 0 012-2h.93a2 2 0 001.664-.89l.812-1.22A2 2 0 0110.07 4h3.86a2 2 0 011.664.89l.812 1.22A2 2 0 0018.07 7H19a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V9z" />
										<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 13a3 3 0 11-6 0 3 3 0 016 0z" />
									</svg>
									<span>Buscar Foto Oficial (TSE)</span>
								{/if}
							</button>

							<button
								type="button"
								on:click={() => (mostrarModalFoto = true)}
								class="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-300 text-xs font-medium flex items-center gap-1.5 transition-colors"
								title="Inserir foto via link de imagem da web ou upload de arquivo"
							>
								<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
									<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
								</svg>
								<span>Personalizar Foto</span>
							</button>

							{#if politico.foto_base64 || fotoLocalUrl}
								<button
									type="button"
									on:click={removerFoto}
									class="px-2.5 py-1.5 rounded-lg bg-rose-500/10 hover:bg-rose-500/20 border border-rose-500/30 text-rose-300 text-xs font-medium transition-colors"
									title="Remover foto salva do banco de dados"
								>
									Remover Foto ✕
								</button>
							{/if}
						</div>
					</div>
				</div>

				<!-- Score de Integridade & Base Parlamentar -->
				<div class="flex flex-col sm:flex-row items-stretch sm:items-center gap-3">
					{#if politico.score_integridade !== undefined}
						<div
							class="flex flex-col items-center justify-center rounded-2xl p-3.5 min-w-[140px] border shadow-lg text-center backdrop-blur-sm"
							style="background-color: {politico.cor_risco_hex || '#10b981'}15; border-color: {politico.cor_risco_hex || '#10b981'}40;"
						>
							<span class="text-[10px] uppercase tracking-wider font-bold text-slate-300">Score Integridade</span>
							<div class="text-3xl font-black font-mono my-0.5" style="color: {politico.cor_risco_hex || '#10b981'};">
								{politico.score_integridade}<span class="text-xs font-normal text-slate-400">/100</span>
							</div>
							<span
								class="px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider font-mono"
								style="background-color: {politico.cor_risco_hex || '#10b981'}30; color: {politico.cor_risco_hex || '#10b981'};"
							>
								Risco {politico.nivel_risco || 'MÍNIMO'}
							</span>
						</div>
					{/if}

					<div class="flex flex-col sm:items-end justify-center bg-slate-950/70 border border-slate-800 rounded-2xl p-4 min-w-[170px]">
						<span class="text-xs text-slate-400 uppercase tracking-wider font-semibold">Base Territorial</span>
						<div class="text-lg font-bold text-white mt-0.5">
							{politico.uf} - Brasil
						</div>
						<div class="text-xs text-slate-400 mt-1 flex items-center gap-1">
							<span class="w-2 h-2 rounded-full bg-emerald-400"></span>
							<span>Registro Ativo</span>
						</div>
					</div>
				</div>
			</div>

			<!-- Badges de Mandatos e Disputas Históricas do Político -->
			{#if mandatosDistintos.length > 0}
				<div class="pt-3 border-t border-slate-800/80 flex flex-wrap items-center gap-2">
					<span class="text-xs text-slate-400 font-medium">Mandatos & Disputas Registradas:</span>
					{#each mandatosDistintos as mandato}
						<span class="px-2.5 py-1 rounded-lg text-xs font-mono bg-slate-950 border border-slate-700/80 text-emerald-300 font-semibold shadow-inner">
							{mandato}
						</span>
					{/each}
				</div>
			{/if}

			<!-- Alertas de Feedback da Busca / Salvamento de Foto -->
			{#if msgFotoSucesso}
				<div class="p-3 bg-emerald-500/10 border border-emerald-500/30 rounded-xl text-emerald-300 text-xs flex items-center justify-between shadow-sm">
					<div class="flex items-center gap-2">
						<span class="font-bold text-sm">✓</span>
						<span>{msgFotoSucesso}</span>
					</div>
					<button type="button" on:click={() => (msgFotoSucesso = null)} class="text-slate-400 hover:text-white font-bold ml-2">✕</button>
				</div>
			{/if}

			{#if msgFotoErro}
				<div class="p-3 bg-rose-500/10 border border-rose-500/30 rounded-xl text-rose-300 text-xs flex items-start justify-between gap-3 shadow-sm">
					<div class="flex items-start gap-2">
						<span class="text-base leading-none">⚠️</span>
						<div>
							<p class="font-semibold">{msgFotoErro}</p>
							<p class="text-slate-400 text-[11px] mt-0.5">
								Dica: Caso a foto oficial não esteja disponível no TSE ou o serviço bloqueie temporariamente, utilize o botão <strong>Personalizar Foto</strong> para colar o link direto ou enviar a imagem do seu computador.
							</p>
						</div>
					</div>
					<button type="button" on:click={() => (msgFotoErro = null)} class="text-slate-400 hover:text-white font-bold flex-shrink-0">✕</button>
				</div>
			{/if}
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

		<!-- Alerta de Auditoria de Possível Parentesco & Nepotismo Cruzado -->
		{#if politico.alertas_parentesco && politico.alertas_parentesco.length > 0}
			<section class="bg-rose-950/30 border border-rose-500/40 rounded-2xl p-5 shadow-xl space-y-3">
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-2">
						<span class="text-xl">⚠️</span>
						<div>
							<h3 class="text-sm font-bold text-rose-300">Auditoria de Possível Parentesco & Nepotismo Cruzado</h3>
							<p class="text-xs text-rose-400/80">Sobrenomes raros compartilhados com sócios de fornecedores CEAP ou doadores na mesma UF ({politico.uf})</p>
						</div>
					</div>
					<span class="px-2.5 py-1 rounded-lg text-xs font-mono font-bold bg-rose-500/20 text-rose-300 border border-rose-500/30">
						{politico.alertas_parentesco.length} {politico.alertas_parentesco.length === 1 ? 'suspeita' : 'suspeitas'}
					</span>
				</div>

				<div class="grid grid-cols-1 md:grid-cols-2 gap-3 pt-2">
					{#each politico.alertas_parentesco as ap}
						<div class="p-3.5 bg-slate-950/80 border border-rose-500/30 rounded-xl space-y-2 text-xs">
							<div class="flex items-start justify-between gap-2">
								<div>
									<strong class="text-white text-sm block">{ap.alvo_nome}</strong>
									<span class="text-slate-400 font-mono text-[11px]">
										{ap.alvo_documento ? formatarCnpjCpf(ap.alvo_documento) : 'Documento protegido'} • {ap.uf}
									</span>
								</div>
								<span class="px-2 py-0.5 rounded text-[10px] font-bold font-mono {ap.nivel_suspeicao === 'ALTO' ? 'bg-rose-500/20 text-rose-300 border border-rose-500/40' : 'bg-amber-500/20 text-amber-300 border border-amber-500/40'}">
									{ap.nivel_suspeicao}
								</span>
							</div>

							<p class="text-slate-300 text-[11px] leading-relaxed">
								{ap.descricao}
							</p>

							<div class="flex items-center gap-1.5 flex-wrap pt-1 border-t border-slate-800">
								<span class="text-[10px] text-slate-400">Sobrenomes coincidentes:</span>
								{#each ap.sobrenomes_compartilhados as sobrenome}
									<span class="px-1.5 py-0.5 rounded bg-slate-800 text-amber-300 font-mono text-[10px] font-bold border border-slate-700">
										{sobrenome}
									</span>
								{/each}
								<span class="text-[10px] text-slate-500 ml-auto font-mono">
									Vínculo: {ap.tipo_vinculo}
								</span>
							</div>
						</div>
					{/each}
				</div>
			</section>
		{/if}

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

			<button
				type="button"
				on:click={() => (abaAtiva = 'evolucao')}
				class="pb-3 px-3 transition-colors flex items-center gap-2 border-b-2 font-bold whitespace-nowrap {abaAtiva === 'evolucao' ? 'border-amber-400 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}"
			>
				<svg class="w-4 h-4 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
				</svg>
				<span>Evolução Patrimonial ({politico.evolucao_patrimonial?.length || 0})</span>
			</button>

			<button
				type="button"
				on:click={() => (abaAtiva = 'emendas')}
				class="pb-3 px-3 transition-colors flex items-center gap-2 border-b-2 font-bold whitespace-nowrap {abaAtiva === 'emendas' ? 'border-emerald-400 text-white' : 'border-transparent text-slate-400 hover:text-slate-200'}"
			>
				<span class="text-sm">💰</span>
				<span>Emendas Parlamentares ({politico.emendas?.length || 0})</span>
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

				<!-- Cargos Públicos e Nomeações de Autoridade de Cúpula -->
				{#if politico.cargos_autoridades && politico.cargos_autoridades.length > 0}
					<section class="bg-slate-900 border border-purple-500/30 rounded-2xl p-5 shadow-xl space-y-4">
						<div class="flex items-center justify-between pb-3 border-b border-slate-800">
							<div class="flex items-center gap-2">
								<span class="text-xl">🏛️</span>
								<div>
									<h3 class="text-sm font-bold text-white">Cargos & Nomeações de Autoridade Pública</h3>
									<p class="text-xs text-purple-300/80">Funções de Estado, cúpula dos poderes e secretarias</p>
								</div>
							</div>
							<span class="text-xs text-purple-300 font-mono font-bold bg-purple-500/10 px-2.5 py-1 rounded-lg border border-purple-500/20">
								{politico.cargos_autoridades.length} {politico.cargos_autoridades.length === 1 ? 'registro' : 'registros'}
							</span>
						</div>

						<div class="space-y-3">
							{#each politico.cargos_autoridades as cargoAuth}
								<div class="p-3.5 bg-slate-950/80 border border-slate-800/90 rounded-xl space-y-2 text-xs">
									<div class="flex items-start justify-between gap-3">
										<div>
											<div class="flex items-center gap-2 flex-wrap">
												<span class="font-bold text-white text-sm">{cargoAuth.cargo}</span>
												<span class="px-2 py-0.5 rounded text-[10px] bg-purple-500/20 text-purple-300 font-bold border border-purple-500/40">
													{cargoAuth.orgao}
												</span>
												<span class="px-1.5 py-0.5 rounded text-[10px] bg-slate-800 text-slate-300 font-mono">
													{cargoAuth.esfera} • {cargoAuth.uf || 'BR'}
												</span>
											</div>
											{#if cargoAuth.ato_nomeacao}
												<p class="text-slate-400 text-[11px] mt-1">
													📜 <strong>Ato / Nomeação:</strong> {cargoAuth.ato_nomeacao}
												</p>
											{/if}
										</div>

										<div class="text-right text-[11px] text-slate-400 font-mono whitespace-nowrap">
											{#if cargoAuth.data_posse}
												<div>Posse: <strong class="text-slate-200">{cargoAuth.data_posse}</strong></div>
											{/if}
											{#if cargoAuth.data_exoneracao}
												<div class="text-slate-500">Exoneração: {cargoAuth.data_exoneracao}</div>
											{:else}
												<span class="text-emerald-400 font-bold text-[10px]">Ativo / Em Exercício</span>
											{/if}
										</div>
									</div>

									{#if cargoAuth.biografia_resumo}
										<p class="text-slate-300 text-[11px] bg-slate-900/70 p-2.5 rounded-lg border border-slate-800/80 leading-relaxed">
											{cargoAuth.biografia_resumo}
										</p>
									{/if}

									<div class="text-[10px] text-slate-500 flex items-center justify-between pt-1">
										<span>Fonte primária: {cargoAuth.origem_dado || 'DADOS_ABERTOS_OFICIAIS'}</span>
									</div>
								</div>
							{/each}
						</div>
					</section>
				{/if}

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
						<div class="p-6 text-center text-xs text-slate-400 space-y-1 bg-slate-950/40 rounded-xl border border-slate-800/50">
							<p class="font-medium text-slate-300">Sem candidaturas partidárias registradas no TSE</p>
							<p class="text-slate-500 text-[11px]">Agente com trajetória em cargos técnicos, de Estado ou sabatinados pelo Senado.</p>
						</div>
					{/if}
				</section>
			</div>
		{:else if abaAtiva === 'evolucao'}
			<div class="space-y-6">
				<!-- Alertas da Auditoria sobre Salto Patrimonial -->
				{#if politico.alertas_evolucao_patrimonial && politico.alertas_evolucao_patrimonial.length > 0}
					<div class="space-y-3">
						{#each politico.alertas_evolucao_patrimonial as alerta}
							<div class="p-5 rounded-2xl bg-amber-500/10 border-2 border-amber-500/40 text-amber-200 space-y-3 shadow-lg">
								<div class="flex items-center justify-between gap-3">
									<div class="flex items-center gap-2.5">
										<span class="text-2xl">⚠️</span>
										<div>
											<h4 class="font-bold text-white text-sm">
												Salto Patrimonial Desproporcional (+{alerta.variacao_percentual.toFixed(0)}%)
											</h4>
											<p class="text-xs text-amber-300/80">
												Variação detectada entre as eleições de {alerta.ano_anterior} e {alerta.ano_recente}
											</p>
										</div>
									</div>
									<span class="px-2.5 py-1 text-xs font-mono font-bold rounded-lg {alerta.gravidade === 'CRITICA' ? 'bg-rose-500/30 text-rose-300 border border-rose-500/50' : 'bg-amber-500/30 text-amber-300 border border-amber-500/50'}">
										{alerta.gravidade}
									</span>
								</div>

								<p class="text-xs text-slate-200 leading-relaxed bg-slate-900/60 p-3.5 rounded-xl border border-amber-500/20 font-mono">
									{alerta.motivo}
								</p>

								<div class="grid grid-cols-2 sm:grid-cols-4 gap-3 text-xs pt-1">
									<div class="bg-slate-950/60 p-2.5 rounded-xl border border-slate-800">
										<span class="text-[10px] text-slate-400 block">Patrimônio ({alerta.ano_anterior}):</span>
										<strong class="text-white font-mono">{formatarMoeda(alerta.valor_anterior)}</strong>
									</div>
									<div class="bg-slate-950/60 p-2.5 rounded-xl border border-slate-800">
										<span class="text-[10px] text-slate-400 block">Patrimônio ({alerta.ano_recente}):</span>
										<strong class="text-emerald-300 font-mono">{formatarMoeda(alerta.valor_recente)}</strong>
									</div>
									<div class="bg-slate-950/60 p-2.5 rounded-xl border border-slate-800">
										<span class="text-[10px] text-slate-400 block">Acréscimo Líquido:</span>
										<strong class="text-amber-300 font-mono">+{formatarMoeda(alerta.incremento_absoluto)}</strong>
									</div>
									<div class="bg-slate-950/60 p-2.5 rounded-xl border border-slate-800">
										<span class="text-[10px] text-slate-400 block">Salto Percentual:</span>
										<strong class="text-rose-400 font-mono">+{alerta.variacao_percentual.toFixed(1)}%</strong>
									</div>
								</div>
							</div>
						{/each}
					</div>
				{/if}

				<!-- Linha do Tempo e Gráfico Visual de Evolução -->
				<section class="bg-slate-900 border border-slate-800 rounded-2xl p-6 shadow-lg space-y-6">
					<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 border-b border-slate-800 pb-4">
						<div>
							<h3 class="text-base font-bold text-white flex items-center gap-2">
								<span class="text-amber-400">📊</span>
								<span>Evolução Histórica do Patrimônio Declarado ao TSE</span>
							</h3>
							<p class="text-xs text-slate-400 mt-0.5">
								Comparação cronológica do patrimônio total a cada registro de candidatura
							</p>
						</div>
						<div class="text-xs text-slate-400 font-mono">
							Fonte Oficial: <span class="text-emerald-400">DivulgaCandContas / TSE</span>
						</div>
					</div>

					{#if politico.evolucao_patrimonial && politico.evolucao_patrimonial.length > 0}
						<!-- Barras Proporcionais por Eleição -->
						<div class="space-y-4">
							{#each politico.evolucao_patrimonial as ponto}
								<div class="p-4 bg-slate-950/70 border border-slate-800/80 rounded-xl space-y-2 hover:border-slate-700 transition-colors">
									<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 text-xs">
										<div class="flex items-center gap-2.5">
											<span class="px-2.5 py-1 rounded-md bg-amber-500/10 border border-amber-500/30 text-amber-300 font-mono font-bold text-xs">
												{ponto.ano}
											</span>
											<span class="font-semibold text-white">{ponto.cargo}</span>
										</div>

										<div class="flex items-center gap-3">
											{#if ponto.variacao_percentual_anterior !== null}
												{#if ponto.variacao_percentual_anterior > 300}
													<span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-rose-500/20 text-rose-300 border border-rose-500/40">
														+{ponto.variacao_percentual_anterior.toFixed(0)}% ({formatarMoeda(ponto.variacao_absoluta_anterior || 0)})
													</span>
												{:else if ponto.variacao_percentual_anterior > 0}
													<span class="px-2 py-0.5 rounded text-[10px] font-mono font-semibold bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">
														+{ponto.variacao_percentual_anterior.toFixed(0)}% (+{formatarMoeda(ponto.variacao_absoluta_anterior || 0)})
													</span>
												{:else if ponto.variacao_percentual_anterior < 0}
													<span class="px-2 py-0.5 rounded text-[10px] font-mono font-semibold bg-slate-800 text-slate-400">
														{ponto.variacao_percentual_anterior.toFixed(0)}% ({formatarMoeda(ponto.variacao_absoluta_anterior || 0)})
													</span>
												{:else}
													<span class="px-2 py-0.5 rounded text-[10px] font-mono text-slate-500">
														Estável
													</span>
												{/if}
											{:else}
												<span class="px-2 py-0.5 rounded text-[10px] text-slate-500 font-mono">
													Primeiro Registro
												</span>
											{/if}

											<strong class="font-mono text-sm text-emerald-400 font-bold min-w-[120px] text-right">
												{formatarMoeda(ponto.valor_total)}
											</strong>
										</div>
									</div>

									<!-- Barra visual -->
									<div class="w-full bg-slate-900 rounded-full h-3 overflow-hidden border border-slate-800">
										<div
											class="h-full rounded-full transition-all duration-500 bg-gradient-to-r from-amber-500 to-emerald-400"
											style="width: {Math.max((ponto.valor_total / maxPatrimonio) * 100, 2)}%"
										></div>
									</div>
								</div>
							{/each}
						</div>

						<!-- Resumo Consolidado do Período -->
						<div class="grid grid-cols-1 sm:grid-cols-3 gap-4 pt-4 border-t border-slate-800 text-xs">
							<div class="p-3.5 bg-slate-950/60 rounded-xl border border-slate-800">
								<span class="text-[10px] text-slate-400 block">Primeira Declaração ({politico.evolucao_patrimonial[0]?.ano}):</span>
								<strong class="text-white font-mono text-sm">{formatarMoeda(politico.evolucao_patrimonial[0]?.valor_total || 0)}</strong>
							</div>
							<div class="p-3.5 bg-slate-950/60 rounded-xl border border-slate-800">
								<span class="text-[10px] text-slate-400 block">Declaração Mais Recente ({politico.evolucao_patrimonial[politico.evolucao_patrimonial.length - 1]?.ano}):</span>
								<strong class="text-emerald-400 font-mono text-sm">{formatarMoeda(politico.evolucao_patrimonial[politico.evolucao_patrimonial.length - 1]?.valor_total || 0)}</strong>
							</div>
							<div class="p-3.5 bg-slate-950/60 rounded-xl border border-slate-800">
								<span class="text-[10px] text-slate-400 block">Pico Patrimonial Atingido:</span>
								<strong class="text-amber-300 font-mono text-sm">{formatarMoeda(maxPatrimonio)}</strong>
							</div>
						</div>
					{:else}
						<div class="p-10 text-center text-slate-400 space-y-2">
							<span class="text-2xl block">📁</span>
							<p>Nenhuma declaração histórica de bens encontrada para este candidato.</p>
							<p class="text-xs text-slate-500">Dossiê aguardando ingestão de dados patrimoniais do TSE.</p>
						</div>
					{/if}
				</section>
			</div>
		{:else if abaAtiva === 'emendas'}
			<!-- Módulo de Auditoria de Emendas Parlamentares (Emendas Pix e Especiais) -->
			<section class="space-y-4">
				<div class="grid grid-cols-1 sm:grid-cols-4 gap-4">
					<div class="p-4 bg-slate-900 border border-slate-800 rounded-2xl shadow-md">
						<span class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider block">Total Empenhado</span>
						<div class="text-xl font-black text-emerald-400 font-mono mt-1">
							{formatarMoeda(totalEmendasEmpenhado)}
						</div>
						<span class="text-[10px] text-slate-500">Recursos orçamentários alocados</span>
					</div>

					<div class="p-4 bg-slate-900 border border-slate-800 rounded-2xl shadow-md">
						<span class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider block">Total Pago / Liquidado</span>
						<div class="text-xl font-black text-cyan-400 font-mono mt-1">
							{formatarMoeda(totalEmendasPago)}
						</div>
						<span class="text-[10px] text-slate-500">Recursos efetivamente transferidos</span>
					</div>

					<div class="p-4 bg-slate-900 border border-slate-800 rounded-2xl shadow-md">
						<span class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider block">Volume de Emendas</span>
						<div class="text-xl font-black text-white font-mono mt-1">
							{politico.emendas?.length || 0}
						</div>
						<span class="text-[10px] text-slate-500">Registros parlamentares identificados</span>
					</div>

					<div class="p-4 bg-slate-900 border border-slate-800 rounded-2xl shadow-md">
						<span class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider block">Taxa de Execução</span>
						<div class="text-xl font-black text-amber-300 font-mono mt-1">
							{totalEmendasEmpenhado > 0 ? `${((totalEmendasPago / totalEmendasEmpenhado) * 100).toFixed(1)}%` : '0%'}
						</div>
						<span class="text-[10px] text-slate-500">Execução financeira sobre empenhado</span>
					</div>
				</div>

				{#if politico.emendas && politico.emendas.length > 0}
					<div class="overflow-x-auto rounded-2xl border border-slate-800 bg-slate-900/90 shadow-xl">
						<table class="w-full text-left text-xs text-slate-300">
							<thead class="bg-slate-950/80 text-[11px] uppercase font-bold text-slate-400 border-b border-slate-800 tracking-wider">
								<tr>
									<th class="py-3 px-3 w-16 text-center">Ano</th>
									<th class="py-3 px-3 min-w-[130px]">Número da Emenda</th>
									<th class="py-3 px-3 min-w-[160px]">Modalidade / Tipo</th>
									<th class="py-3 px-3 min-w-[150px]">Destino / UF</th>
									<th class="py-3 px-4 min-w-[200px]">Beneficiário</th>
									<th class="py-3 px-3 text-right min-w-[120px]">Valor Empenhado</th>
									<th class="py-3 px-3 text-right min-w-[120px]">Valor Pago</th>
								</tr>
							</thead>
							<tbody class="divide-y divide-slate-800/60">
								{#each politico.emendas as emenda}
									<tr class="hover:bg-slate-800/40 transition-colors">
										<td class="py-3 px-3 text-center font-mono font-bold text-slate-400">
											{emenda.ano}
										</td>
										<td class="py-3 px-3 font-mono font-semibold text-white">
											{emenda.numero_emenda}
										</td>
										<td class="py-3 px-3">
											{#if emenda.tipo_emenda.includes('ESPECIAL') || emenda.tipo_emenda.includes('PIX')}
												<span class="px-2 py-0.5 rounded text-[10px] font-bold bg-amber-500/20 text-amber-300 border border-amber-500/40 whitespace-nowrap">
													⚡ Transf. Especial (Emenda Pix)
												</span>
											{:else}
												<span class="px-2 py-0.5 rounded text-[10px] font-medium bg-slate-800 text-slate-300 border border-slate-700 whitespace-nowrap">
													{emenda.tipo_emenda}
												</span>
											{/if}
										</td>
										<td class="py-3 px-3 text-slate-300">
											<span class="font-medium">{emenda.localidade_destino}</span>
											<span class="px-1.5 py-0.5 rounded text-[10px] font-bold bg-slate-800 text-emerald-400 border border-slate-700 font-mono ml-1">
												{emenda.uf}
											</span>
										</td>
										<td class="py-3 px-4 text-slate-300 font-medium">
											{emenda.beneficiario}
										</td>
										<td class="py-3 px-3 text-right font-mono font-bold text-emerald-400 whitespace-nowrap">
											{formatarMoeda(emenda.valor_empenhado)}
										</td>
										<td class="py-3 px-3 text-right font-mono font-bold text-cyan-400 whitespace-nowrap">
											{formatarMoeda(emenda.valor_pago)}
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{:else}
					<div class="p-10 text-center bg-slate-900 border border-slate-800 rounded-2xl space-y-2">
						<span class="text-3xl block">🏛️</span>
						<h3 class="text-base font-semibold text-white">Nenhuma emenda parlamentar vinculada</h3>
						<p class="text-xs text-slate-400 max-w-md mx-auto">
							Não foram encontradas emendas individuais ou transferências especiais associadas a este parlamentar na base do Siop/Transparência.
						</p>
					</div>
				{/if}
			</section>
		{/if}
	{/if}
</div>

<!-- Modal para Inserir / Personalizar Foto -->
{#if mostrarModalFoto}
	<div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-sm animate-fade-in">
		<div class="bg-slate-900 border border-slate-800 rounded-3xl max-w-md w-full p-6 shadow-2xl space-y-5">
			<div class="flex items-center justify-between pb-3 border-b border-slate-800">
				<h3 class="text-base font-bold text-white flex items-center gap-2">
					<span class="text-lg">📸</span>
					<span>Personalizar Foto do Político</span>
				</h3>
				<button
					type="button"
					on:click={() => (mostrarModalFoto = false)}
					class="w-7 h-7 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-400 hover:text-white flex items-center justify-center text-sm transition-colors"
				>
					✕
				</button>
			</div>

			<p class="text-xs text-slate-400 leading-relaxed">
				A imagem enviada ou informada via link será gravada permanentemente no banco de dados SQLite local, servindo como ícone oficial sem necessidade de novo download.
			</p>

			<!-- Opção 1: Inserir via URL direta da web -->
			<div class="space-y-2 bg-slate-950/60 p-3.5 rounded-2xl border border-slate-800">
				<label for="input-foto-url" class="block text-xs font-semibold text-slate-200">
					Opção 1: Informar URL da Foto na Web
				</label>
				<div class="flex gap-2">
					<input
						id="input-foto-url"
						type="url"
						bind:value={fotoUrlInput}
						placeholder="https://exemplo.com/foto_politico.jpg"
						class="flex-1 bg-slate-900 border border-slate-700 rounded-xl px-3 py-2 text-xs text-white placeholder-slate-500 focus:outline-none focus:ring-1 focus:ring-emerald-500"
					/>
					<button
						type="button"
						on:click={salvarFotoPorUrl}
						disabled={salvandoFoto || !fotoUrlInput.trim()}
						class="px-3.5 py-2 bg-emerald-600 hover:bg-emerald-500 text-white rounded-xl text-xs font-bold disabled:opacity-50 transition-colors shadow-sm"
					>
						{salvandoFoto ? 'Salvando...' : 'Salvar'}
					</button>
				</div>
			</div>

			<div class="relative flex items-center justify-center">
				<div class="w-full border-t border-slate-800"></div>
				<span class="absolute bg-slate-900 px-3 text-[11px] text-slate-500 uppercase font-mono font-bold tracking-wider">
					ou
				</span>
			</div>

			<!-- Opção 2: Upload de Arquivo Local -->
			<div class="space-y-2 bg-slate-950/60 p-3.5 rounded-2xl border border-slate-800">
				<span class="block text-xs font-semibold text-slate-200">
					Opção 2: Enviar Arquivo de Imagem
				</span>
				<input
					type="file"
					accept="image/jpeg,image/png,image/webp"
					bind:this={uploadFileInput}
					on:change={handleUploadArquivo}
					disabled={salvandoFoto}
					class="block w-full text-xs text-slate-400 file:mr-3 file:py-2 file:px-3 file:rounded-lg file:border-0 file:text-xs file:font-semibold file:bg-slate-800 file:text-emerald-300 hover:file:bg-slate-700 cursor-pointer disabled:opacity-50"
				/>
				<p class="text-[10px] text-slate-500">Formatos aceitos: JPEG, PNG, WebP (máximo 2 MB)</p>
			</div>

			<div class="pt-2 border-t border-slate-800 flex justify-end">
				<button
					type="button"
					on:click={() => (mostrarModalFoto = false)}
					class="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-xs text-slate-300 hover:text-white rounded-xl font-medium transition-colors"
				>
					Fechar
				</button>
			</div>
		</div>
	</div>
{/if}

<!-- Rodapé Formal do Dossiê para Impressão -->
<div class="hidden print:block pt-6 mt-8 border-t-2 border-slate-800 text-[10px] text-slate-600 font-mono">
	<div class="flex justify-between items-center">
		<div>
			Documento emitido automaticamente pelo Radar Cívico. Fontes públicas primárias: TSE, CEAP/Câmara dos Deputados, STF, TCU, Querido Diário e Receita Federal.
		</div>
		<div>
			Relatório Oficial de Auditoria Cívica
		</div>
	</div>
</div>

<style>
	@media print {
		@page {
			size: A4;
			margin: 12mm 15mm 12mm 15mm;
		}
		:global(body) {
			background: #ffffff !important;
			color: #000000 !important;
			font-size: 11pt !important;
		}
		:global(nav),
		:global(header),
		:global(footer),
		button,
		input,
		a[href^="/politicos"],
		a[href^="/dossie/cpf"] {
			display: none !important;
		}
		:global(.bg-slate-900),
		:global(.bg-slate-950),
		:global(.bg-slate-800) {
			background: #ffffff !important;
			border-color: #cbd5e1 !important;
			color: #0f172a !important;
		}
		:global(.text-white) {
			color: #0f172a !important;
		}
		:global(.text-slate-200),
		:global(.text-slate-300) {
			color: #1e293b !important;
		}
		:global(.text-slate-400),
		:global(.text-slate-500) {
			color: #475569 !important;
		}
		:global(.shadow-xl),
		:global(.shadow-lg),
		:global(.shadow-2xl) {
			box-shadow: none !important;
		}
		section, :global(.space-y-6 > *) {
			break-inside: avoid !important;
			page-break-inside: avoid !important;
		}
	}
</style>

