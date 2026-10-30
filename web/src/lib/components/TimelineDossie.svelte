<script lang="ts">
	import type { PoliticoDetalheResponse } from '#lib/types';

	export let politico: PoliticoDetalheResponse;

	type CategoriaEvento = 'todos' | 'eleicao' | 'patrimonio' | 'mandato' | 'ceap' | 'emenda' | 'alerta';

	interface EventoTimeline {
		id: string;
		dataOuAno: string;
		sortKey: number; // YYYYMMDD ou YYYY0000 para ordenação
		categoria: 'eleicao' | 'patrimonio' | 'mandato' | 'ceap' | 'emenda' | 'alerta';
		titulo: string;
		descricao: string;
		detalheExtra?: string;
		valorFormatado?: string;
		severidade?: 'normal' | 'alerta' | 'perigo' | 'sucesso';
		icone?: string;
	}

	function formatarMoeda(val: number): string {
		return new Intl.NumberFormat('pt-BR', { style: 'currency', currency: 'BRL' }).format(val);
	}

	function formatarDataBR(dataIso: string): string {
		if (!dataIso) return '';
		const partes = dataIso.split('T')[0].split('-');
		if (partes.length === 3) {
			return `${partes[2]}/${partes[1]}/${partes[0]}`;
		}
		return dataIso;
	}

	function normalizarDataParaSortKey(dataStr: string | null | undefined, anoFallback?: number): number {
		if (dataStr) {
			const limpa = dataStr.split('T')[0].replace(/\D/g, '');
			if (limpa.length >= 8) {
				return Number(limpa.slice(0, 8));
			}
			if (limpa.length === 4) {
				return Number(limpa) * 10000;
			}
		}
		if (anoFallback) {
			return anoFallback * 10000;
		}
		return 19000101;
	}

	let filtroCategoria: CategoriaEvento = 'todos';
	let ordemDecrescente = true;
	let buscaTexto = '';

	$: eventos = construirEventos(politico);

	$: eventosFiltrados = eventos
		.filter((e) => {
			if (filtroCategoria !== 'todos' && e.categoria !== filtroCategoria) {
				return false;
			}
			if (buscaTexto.trim().length > 0) {
				const termo = buscaTexto.toLowerCase();
				return (
					e.titulo.toLowerCase().includes(termo) ||
					e.descricao.toLowerCase().includes(termo) ||
					(e.detalheExtra && e.detalheExtra.toLowerCase().includes(termo)) ||
					(e.valorFormatado && e.valorFormatado.toLowerCase().includes(termo))
				);
			}
			return true;
		})
		.sort((a, b) => (ordemDecrescente ? b.sortKey - a.sortKey : a.sortKey - b.sortKey));

	function construirEventos(p: PoliticoDetalheResponse): EventoTimeline[] {
		const lista: EventoTimeline[] = [];

		// 1. Candidaturas
		if (p.candidaturas && p.candidaturas.length > 0) {
			p.candidaturas.forEach((c) => {
				const situacao = c.situacao_totalizacao || 'Participação';
				const foiEleito = /eleito/i.test(situacao);
				lista.push({
					id: `cand-${c.id}-${c.ano_eleicao}`,
					dataOuAno: `${c.ano_eleicao}`,
					sortKey: c.ano_eleicao * 10000 + 1001,
					categoria: 'eleicao',
					titulo: `Eleição ${c.ano_eleicao} • ${c.cargo}`,
					descricao: `${c.sigla_partido} (${c.uf}) ${c.municipio ? `• ${c.municipio}` : ''} • Resultado: ${situacao}`,
					detalheExtra: c.numero_urna ? `Nº Urna: ${c.numero_urna}` : undefined,
					valorFormatado: c.total_bens_declarados > 0 ? `Bens declarados: ${formatarMoeda(c.total_bens_declarados)}` : undefined,
					severidade: foiEleito ? 'sucesso' : 'normal',
					icone: '🗳️'
				});
			});
		}

		// 2. Evolução Patrimonial
		if (p.evolucao_patrimonial && p.evolucao_patrimonial.length > 0) {
			p.evolucao_patrimonial.forEach((ev, idx) => {
				const variacaoStr = ev.variacao_percentual_anterior !== null && ev.variacao_percentual_anterior !== undefined
					? `${ev.variacao_percentual_anterior > 0 ? '+' : ''}${ev.variacao_percentual_anterior.toFixed(1)}% em relação ao pleito anterior`
					: 'Primeira declaração identificada';
				lista.push({
					id: `patr-${ev.ano}-${idx}`,
					dataOuAno: `${ev.ano}`,
					sortKey: ev.ano * 10000 + 1002,
					categoria: 'patrimonio',
					titulo: `Declaração de Patrimônio (${ev.ano})`,
					descricao: `Patrimônio total declarado à Justiça Eleitoral: ${formatarMoeda(ev.valor_total)}`,
					detalheExtra: variacaoStr,
					valorFormatado: formatarMoeda(ev.valor_total),
					severidade: ev.variacao_percentual_anterior && ev.variacao_percentual_anterior > 200 ? 'alerta' : 'normal',
					icone: '💼'
				});
			});
		}

		// 3. Cargos e Autoridades (Posse / Exoneração)
		if (p.cargos_autoridades && p.cargos_autoridades.length > 0) {
			p.cargos_autoridades.forEach((ca) => {
				if (ca.data_posse) {
					lista.push({
						id: `cargo-posse-${ca.id}`,
						dataOuAno: formatarDataBR(ca.data_posse),
						sortKey: normalizarDataParaSortKey(ca.data_posse),
						categoria: 'mandato',
						titulo: `Posse: ${ca.cargo}`,
						descricao: `${ca.orgao} • Esfera ${ca.esfera}${ca.uf ? ` (${ca.uf})` : ''}`,
						detalheExtra: ca.ato_nomeacao ? `Ato: ${ca.ato_nomeacao}` : undefined,
						severidade: 'normal',
						icone: '🏛️'
					});
				}
				if (ca.data_exoneracao) {
					lista.push({
						id: `cargo-exon-${ca.id}`,
						dataOuAno: formatarDataBR(ca.data_exoneracao),
						sortKey: normalizarDataParaSortKey(ca.data_exoneracao),
						categoria: 'mandato',
						titulo: `Término / Exoneração: ${ca.cargo}`,
						descricao: `${ca.orgao} (${ca.esfera})`,
						severidade: 'normal',
						icone: '📜'
					});
				}
			});
		}

		// 4. Emendas Parlamentares
		if (p.emendas && p.emendas.length > 0) {
			// Agrupar emendas por ano para síntese equilibrada
			const emendasPorAno: Record<number, { empenhado: number; pago: number; count: number; destinos: Set<string> }> = {};
			p.emendas.forEach((em) => {
				const ano = em.ano || 2024;
				if (!emendasPorAno[ano]) {
					emendasPorAno[ano] = { empenhado: 0, pago: 0, count: 0, destinos: new Set() };
				}
				emendasPorAno[ano].empenhado += em.valor_empenhado || 0;
				emendasPorAno[ano].pago += em.valor_pago || 0;
				emendasPorAno[ano].count += 1;
				if (em.localidade_destino) emendasPorAno[ano].destinos.add(em.localidade_destino);
			});

			Object.entries(emendasPorAno).forEach(([anoStr, dados]) => {
				const ano = Number(anoStr);
				const destinosAmor = Array.from(dados.destinos).slice(0, 3).join(', ');
				lista.push({
					id: `emendas-ano-${ano}`,
					dataOuAno: `${ano}`,
					sortKey: ano * 10000 + 1231,
					categoria: 'emenda',
					titulo: `Emendas do Exercício ${ano} (${dados.count} indicações)`,
					descricao: `Execução orçamentária: ${formatarMoeda(dados.pago)} pagos de ${formatarMoeda(dados.empenhado)} empenhados.`,
					detalheExtra: destinosAmor ? `Principais localidades beneficiadas: ${destinosAmor}` : undefined,
					valorFormatado: formatarMoeda(dados.pago),
					severidade: 'sucesso',
					icone: '💰'
				});
			});
		}

		// 5. Despesas CEAP Recentes / Notáveis
		if (p.despesas_recentes && p.despesas_recentes.length > 0) {
			p.despesas_recentes.slice(0, 25).forEach((d) => {
				lista.push({
					id: `ceap-${d.id}`,
					dataOuAno: formatarDataBR(d.data_emissao),
					sortKey: normalizarDataParaSortKey(d.data_emissao),
					categoria: 'ceap',
					titulo: `Despesa CEAP • ${d.fornecedor_nome}`,
					descricao: `${d.categoria_despesa} • Doc: ${d.numero_documento || 'S/N'}`,
					detalheExtra: d.detalhes_litros ? `Volume faturado: ${d.detalhes_litros} L` : undefined,
					valorFormatado: formatarMoeda(d.valor_liquido),
					severidade: d.flag_anomalia ? 'perigo' : 'normal',
					icone: d.flag_anomalia ? '⚠️' : '🧾'
				});
			});
		}

		// 6. Alertas e Auditorias
		if (p.alertas_evolucao_patrimonial && p.alertas_evolucao_patrimonial.length > 0) {
			p.alertas_evolucao_patrimonial.forEach((a, i) => {
				lista.push({
					id: `alerta-evolucao-${i}`,
					dataOuAno: `${a.ano_recente}`,
					sortKey: a.ano_recente * 10000 + 1003,
					categoria: 'alerta',
					titulo: `Auditoria: Variação Patrimonial Atípica (+${a.variacao_percentual.toFixed(0)}%)`,
					descricao: `${a.motivo} (de ${formatarMoeda(a.valor_anterior)} em ${a.ano_anterior} para ${formatarMoeda(a.valor_recente)} em ${a.ano_recente})`,
					detalheExtra: `Incremento de ${formatarMoeda(a.incremento_absoluto)}`,
					severidade: 'perigo',
					icone: '🚨'
				});
			});
		}

		if (p.alertas_auxilio && p.alertas_auxilio.length > 0) {
			p.alertas_auxilio.forEach((a) => {
				lista.push({
					id: `alerta-auxilio-${a.id}`,
					dataOuAno: 'Período Pandemia',
					sortKey: 20200501,
					categoria: 'alerta',
					titulo: 'Auditoria: Benefício Emergencial Identificado',
					descricao: a.motivo,
					detalheExtra: a.detalhes || undefined,
					severidade: 'perigo',
					icone: '🚨'
				});
			});
		}

		if (p.alertas_parentesco && p.alertas_parentesco.length > 0) {
			p.alertas_parentesco.forEach((ap, i) => {
				lista.push({
					id: `alerta-parentesco-${i}`,
					dataOuAno: 'Auditoria Ativa',
					sortKey: 20250101,
					categoria: 'alerta',
					titulo: `Vínculo de Parentesco / Nepotismo Cruzado (${ap.nivel_suspeicao})`,
					descricao: `Alvo: ${ap.alvo_nome} (${ap.tipo_vinculo}) • ${ap.descricao}`,
					severidade: ap.nivel_suspeicao === 'ALTO' ? 'perigo' : 'alerta',
					icone: '👥'
				});
			});
		}

		return lista;
	}
</script>

<div class="space-y-6">
	<!-- Painel de Filtros e Controles da Timeline -->
	<div class="bg-slate-900 border border-slate-800 rounded-2xl p-4 sm:p-5 shadow-lg flex flex-col md:flex-row md:items-center justify-between gap-4">
		<!-- Filtro por Categoria -->
		<div class="flex flex-wrap items-center gap-1.5 text-xs">
			<button
				type="button"
				on:click={() => (filtroCategoria = 'todos')}
				class="px-3 py-1.5 rounded-lg font-semibold transition-colors {filtroCategoria === 'todos' ? 'bg-emerald-500 text-slate-950 shadow-md' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'}"
			>
				Todos ({eventos.length})
			</button>
			<button
				type="button"
				on:click={() => (filtroCategoria = 'eleicao')}
				class="px-3 py-1.5 rounded-lg font-semibold transition-colors {filtroCategoria === 'eleicao' ? 'bg-purple-500 text-white shadow-md' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'}"
			>
				🗳️ Eleições
			</button>
			<button
				type="button"
				on:click={() => (filtroCategoria = 'patrimonio')}
				class="px-3 py-1.5 rounded-lg font-semibold transition-colors {filtroCategoria === 'patrimonio' ? 'bg-amber-500 text-slate-950 shadow-md' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'}"
			>
				💼 Patrimônio
			</button>
			<button
				type="button"
				on:click={() => (filtroCategoria = 'mandato')}
				class="px-3 py-1.5 rounded-lg font-semibold transition-colors {filtroCategoria === 'mandato' ? 'bg-cyan-500 text-slate-950 shadow-md' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'}"
			>
				🏛️ Cargos & Posse
			</button>
			<button
				type="button"
				on:click={() => (filtroCategoria = 'emenda')}
				class="px-3 py-1.5 rounded-lg font-semibold transition-colors {filtroCategoria === 'emenda' ? 'bg-emerald-500 text-slate-950 shadow-md' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'}"
			>
				💰 Emendas
			</button>
			<button
				type="button"
				on:click={() => (filtroCategoria = 'ceap')}
				class="px-3 py-1.5 rounded-lg font-semibold transition-colors {filtroCategoria === 'ceap' ? 'bg-indigo-500 text-white shadow-md' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'}"
			>
				🧾 Despesas CEAP
			</button>
			<button
				type="button"
				on:click={() => (filtroCategoria = 'alerta')}
				class="px-3 py-1.5 rounded-lg font-semibold transition-colors {filtroCategoria === 'alerta' ? 'bg-rose-500 text-white shadow-md' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'}"
			>
				🚨 Alertas & Auditoria
			</button>
		</div>

		<!-- Busca Rápida e Alternância de Ordem -->
		<div class="flex items-center gap-2">
			<div class="relative flex-1 sm:w-56">
				<input
					type="text"
					bind:value={buscaTexto}
					placeholder="Filtrar eventos..."
					class="w-full text-xs bg-slate-950 border border-slate-700 rounded-xl px-3 py-2 text-white placeholder-slate-500 focus:outline-none focus:border-emerald-400"
				/>
				{#if buscaTexto}
					<button
						type="button"
						on:click={() => (buscaTexto = '')}
						class="absolute right-2.5 top-2 text-slate-400 hover:text-white text-xs font-bold"
					>
						✕
					</button>
				{/if}
			</div>

			<button
				type="button"
				on:click={() => (ordemDecrescente = !ordemDecrescente)}
				class="px-3 py-2 bg-slate-800 hover:bg-slate-700 border border-slate-700 text-xs font-semibold text-slate-200 rounded-xl transition-colors flex items-center gap-1.5 whitespace-nowrap"
				title="Alternar entre mais recentes ou mais antigos primeiro"
			>
				<span>{ordemDecrescente ? '⬇️ Mais recentes' : '⬆️ Mais antigos'}</span>
			</button>
		</div>
	</div>

	<!-- Lista da Timeline Cronológica -->
	{#if eventosFiltrados.length === 0}
		<div class="text-center py-12 bg-slate-900 border border-slate-800 rounded-2xl text-slate-400 text-sm">
			<p class="font-semibold text-white">Nenhum evento histórico encontrado com os filtros atuais.</p>
			<p class="text-xs mt-1 text-slate-500">Tente selecionar outra categoria ou limpar o termo de busca.</p>
		</div>
	{:else}
		<div class="relative pl-6 sm:pl-8 space-y-6 before:absolute before:left-2.5 sm:before:left-3.5 before:top-3 before:bottom-3 before:w-0.5 before:bg-gradient-to-b before:from-emerald-400 before:via-slate-700 before:to-slate-800">
			{#each eventosFiltrados as evento (evento.id)}
				<div class="relative group">
					<!-- Ponto / Marcador Circular -->
					<div
						class="absolute -left-6 sm:-left-8 top-3 w-5 h-5 rounded-full border-2 flex items-center justify-center text-[10px] shadow-md transition-transform group-hover:scale-125 {evento.severidade === 'perigo'
							? 'bg-rose-950 border-rose-500 text-rose-300'
							: evento.severidade === 'alerta'
								? 'bg-amber-950 border-amber-500 text-amber-300'
								: evento.severidade === 'sucesso'
									? 'bg-emerald-950 border-emerald-500 text-emerald-300'
									: 'bg-slate-900 border-slate-600 text-slate-300'}"
					>
						<span class="w-1.5 h-1.5 rounded-full {evento.severidade === 'perigo' ? 'bg-rose-400 animate-pulse' : evento.severidade === 'sucesso' ? 'bg-emerald-400' : 'bg-slate-400'}"></span>
					</div>

					<!-- Card do Evento -->
					<div
						class="bg-slate-900/90 hover:bg-slate-850 border rounded-2xl p-4 sm:p-5 shadow-md transition-all duration-200 {evento.severidade === 'perigo'
							? 'border-rose-500/40 bg-rose-950/10'
							: evento.severidade === 'alerta'
								? 'border-amber-500/40 bg-amber-950/10'
								: 'border-slate-800 hover:border-slate-700'}"
					>
						<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 border-b border-slate-800/80 pb-2.5 mb-2.5">
							<div class="flex items-center gap-2">
								<span class="text-base">{evento.icone || '📌'}</span>
								<h3 class="text-sm font-bold text-white group-hover:text-emerald-400 transition-colors">
									{evento.titulo}
								</h3>
							</div>

							<div class="flex items-center gap-2">
								{#if evento.valorFormatado}
									<span class="px-2.5 py-0.5 rounded-lg text-xs font-mono font-bold bg-slate-950 border border-slate-800 text-emerald-300">
										{evento.valorFormatado}
									</span>
								{/if}
								<span class="text-xs font-mono font-semibold px-2 py-0.5 rounded bg-slate-800 text-slate-300">
									{evento.dataOuAno}
								</span>
							</div>
						</div>

						<p class="text-xs sm:text-sm text-slate-300 leading-relaxed">
							{evento.descricao}
						</p>

						{#if evento.detalheExtra}
							<div class="mt-2.5 pt-2 border-t border-slate-800/60 text-xs text-slate-400 font-mono flex items-center gap-1.5">
								<span class="text-slate-500">ℹ️</span>
								<span>{evento.detalheExtra}</span>
							</div>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
