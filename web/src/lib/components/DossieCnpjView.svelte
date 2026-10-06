<script lang="ts">
	import type { DossieCnpj } from '$lib/types';

	export let dossie: DossieCnpj;

	function formatarMoeda(valor: number): string {
		return new Intl.NumberFormat('pt-BR', { style: 'currency', currency: 'BRL' }).format(valor);
	}

	function formatarData(dataStr: string | null): string {
		if (!dataStr) return 'Não informada';
		const partes = dataStr.split('T')[0].split('-');
		if (partes.length === 3) {
			return `${partes[2]}/${partes[1]}/${partes[0]}`;
		}
		return dataStr;
	}

	function corSeveridade(sev: string): { bg: string; text: string; border: string; pulse: boolean } {
		switch (sev.toUpperCase()) {
			case 'CRITICA':
			case 'CRITICO':
				return { bg: 'bg-rose-500/20', text: 'text-rose-300', border: 'border-rose-500/40', pulse: true };
			case 'ALTA':
			case 'ALTO':
				return { bg: 'bg-amber-500/20', text: 'text-amber-300', border: 'border-amber-500/40', pulse: false };
			case 'MEDIA':
			case 'MEDIO':
				return { bg: 'bg-yellow-500/20', text: 'text-yellow-300', border: 'border-yellow-500/40', pulse: false };
			case 'BAIXA':
			case 'BAIXO':
				return { bg: 'bg-emerald-500/20', text: 'text-emerald-300', border: 'border-emerald-500/40', pulse: false };
			default:
				return { bg: 'bg-slate-700/30', text: 'text-slate-300', border: 'border-slate-700', pulse: false };
		}
	}
</script>

<div class="space-y-8">
	<!-- Cabeçalho do Dossiê CNPJ -->
	<div class="bg-slate-800/80 border border-slate-700/80 rounded-2xl p-6 sm:p-8 shadow-xl">
		<div class="flex flex-col md:flex-row md:items-start justify-between gap-6">
			<div class="space-y-3">
				<div class="flex flex-wrap items-center gap-2.5">
					<span class="px-2.5 py-1 text-xs font-bold rounded-lg bg-cyan-500/10 border border-cyan-500/30 text-cyan-400">
						PESSOA JURÍDICA (CNPJ)
					</span>
					<span class="px-3 py-1 text-xs font-mono rounded-lg bg-slate-900 border border-slate-700 text-slate-300">
						{dossie.cnpj_formatado}
					</span>
					<span class="px-2.5 py-1 text-xs rounded-lg bg-emerald-500/10 border border-emerald-500/30 text-emerald-400">
						{dossie.situacao_cadastral}
					</span>
					{#if dossie.total_alertas > 0}
						{@const badgeRisco = corSeveridade(dossie.score_risco)}
						<span class={`px-3 py-1 text-xs font-bold rounded-full border ${badgeRisco.bg} ${badgeRisco.text} ${badgeRisco.border} flex items-center gap-1.5 shadow-sm`}>
							{#if badgeRisco.pulse}
								<span class="w-2 h-2 rounded-full bg-rose-400 animate-pulse"></span>
							{/if}
							Risco {dossie.score_risco} ({dossie.total_alertas} {dossie.total_alertas === 1 ? 'anomalia' : 'anomalias'})
						</span>
					{/if}
				</div>

				<h1 class="text-2xl sm:text-3xl font-extrabold text-white tracking-tight">
					{dossie.razao_social}
				</h1>
				<p class="text-xs sm:text-sm text-slate-400">
					Dossiê analítico integrado com Receita Federal (QSA), Câmara dos Deputados (CEAP), Compras Públicas (PNCP) e Tribunal Superior Eleitoral (TSE).
				</p>
			</div>

			<div class="flex items-center gap-2 self-start flex-shrink-0">
				<a
					href={`/grafo/cnpj_${dossie.cnpj}?grau=2`}
					class="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-slate-200 text-xs font-semibold rounded-xl border border-slate-600 transition-colors flex items-center gap-2"
				>
					<svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
					</svg>
					<span>Explorar no Grafo</span>
				</a>
			</div>
		</div>

		<!-- Cartões de Métricas Rápidas -->
		<div class="grid grid-cols-2 lg:grid-cols-4 gap-3 sm:gap-4 mt-6 pt-6 border-t border-slate-700/60">
			<div class="bg-slate-900/60 p-4 rounded-xl border border-slate-700/40">
				<span class="text-xs text-slate-400 block font-medium">Faturamento CEAP</span>
				<span class="text-lg sm:text-xl font-bold text-emerald-400 mt-1 block">
					{formatarMoeda(dossie.ceap.total_faturado)}
				</span>
				<span class="text-[11px] text-slate-500">{dossie.ceap.total_notas} notas para gabinetes</span>
			</div>

			<div class="bg-slate-900/60 p-4 rounded-xl border border-slate-700/40">
				<span class="text-xs text-slate-400 block font-medium">Contratos PNCP</span>
				<span class="text-lg sm:text-xl font-bold text-cyan-400 mt-1 block">
					{formatarMoeda(dossie.pncp.total_contratado)}
				</span>
				<span class="text-[11px] text-slate-500">{dossie.pncp.total_contratos} contratos públicos</span>
			</div>

			<div class="bg-slate-900/60 p-4 rounded-xl border border-slate-700/40">
				<span class="text-xs text-slate-400 block font-medium">Doações TSE (Sócios)</span>
				<span class="text-lg sm:text-xl font-bold text-amber-400 mt-1 block">
					{formatarMoeda(dossie.tse.total_doacoes_socios)}
				</span>
				<span class="text-[11px] text-slate-500">{dossie.tse.doacoes.length} repasses eleitorais</span>
			</div>

			<div class="bg-slate-900/60 p-4 rounded-xl border border-slate-700/40">
				<span class="text-xs text-slate-400 block font-medium">Quadro Societário (QSA)</span>
				<span class="text-lg sm:text-xl font-bold text-purple-400 mt-1 block">
					{dossie.qsa.total_socios}
				</span>
				<span class="text-[11px] text-slate-500">{dossie.qsa.empresas_interligadas.length} empresas interligadas</span>
			</div>
		</div>
	</div>

	<!-- Seção de Alertas e Anomalias Detectadas -->
	{#if dossie.alertas.length > 0}
		<div class="bg-slate-800/80 border border-slate-700/80 rounded-2xl p-6 shadow-xl space-y-4">
			<div class="flex items-center gap-3">
				<div class="w-8 h-8 rounded-lg bg-rose-500/10 border border-rose-500/30 flex items-center justify-center text-rose-400">
					<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
					</svg>
				</div>
				<div>
					<h2 class="text-base font-bold text-white">Alertas e Anomalias Identificadas pelo Auditor</h2>
					<p class="text-xs text-slate-400">Cruzamentos determinísticos e heurísticos entre doações, contratações e cotas</p>
				</div>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-2 gap-3 pt-2">
				{#each dossie.alertas as alerta}
					{@const b = corSeveridade(alerta.severidade)}
					<div class={`p-4 rounded-xl border ${b.bg} ${b.border} space-y-2`}>
						<div class="flex items-center justify-between gap-2">
							<span class={`text-[10px] font-bold px-2 py-0.5 rounded-full uppercase tracking-wider border ${b.border} ${b.text}`}>
								{alerta.severidade}
							</span>
							<span class="text-[11px] text-slate-400">{alerta.fonte}</span>
						</div>
						<h3 class="font-semibold text-white text-sm">{alerta.titulo}</h3>
						<p class="text-xs text-slate-300 leading-relaxed">{alerta.descricao}</p>
						{#if alerta.valor_envolvido}
							<p class="text-xs font-mono font-semibold text-emerald-400 pt-1">
								Valor Envolvido: {formatarMoeda(alerta.valor_envolvido)}
							</p>
						{/if}
					</div>
				{/each}
			</div>
		</div>
	{/if}

	<!-- Grade de Painéis Interligados (4 Painéis) -->
	<div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
		<!-- Painel 1: Societário (QSA - Receita Federal) -->
		<div class="bg-slate-800/80 border border-slate-700/80 rounded-2xl p-6 shadow-xl flex flex-col justify-between">
			<div class="space-y-4">
				<div class="flex items-center justify-between border-b border-slate-700/60 pb-3">
					<div class="flex items-center gap-2.5">
						<div class="w-8 h-8 rounded-lg bg-purple-500/10 border border-purple-500/30 flex items-center justify-center text-purple-400">
							<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
							</svg>
						</div>
						<div>
							<h2 class="text-base font-bold text-white">1. Quadro Societário (QSA)</h2>
							<p class="text-xs text-slate-400">Sócios, Administradores e Vínculos Cruzados</p>
						</div>
					</div>
					<span class="text-xs px-2.5 py-1 rounded-full bg-slate-900 text-purple-300 font-mono">
						{dossie.qsa.total_socios} sócios
					</span>
				</div>

				{#if dossie.qsa.socios.length === 0}
					<p class="text-xs text-slate-500 py-4 italic">Nenhum sócio catalogado no QSA para este CNPJ.</p>
				{:else}
					<div class="space-y-3 max-h-96 overflow-y-auto pr-1">
						{#each dossie.qsa.socios as socio}
							<div class="p-3.5 rounded-xl bg-slate-900/60 border border-slate-700/50 space-y-2">
								<div class="flex items-center justify-between gap-2">
									<a
										href={`/dossie/cpf/${encodeURIComponent(socio.documento_mascarado.replace(/\D/g, '') || socio.nome)}`}
										class="text-sm font-semibold text-purple-300 hover:text-purple-200 hover:underline flex items-center gap-1.5"
									>
										<span>{socio.nome}</span>
										<svg class="w-3.5 h-3.5 text-slate-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
											<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
										</svg>
									</a>
									<span class="text-[11px] font-mono px-2 py-0.5 rounded bg-slate-800 text-slate-400">
										{socio.tipo} • {socio.documento_mascarado}
									</span>
								</div>
								{#if socio.qualificacao}
									<p class="text-xs text-slate-400">Qualificação: <strong class="text-slate-300">{socio.qualificacao}</strong></p>
								{/if}

								{#if socio.outras_empresas.length > 0}
									<div class="pt-2 border-t border-slate-800/80">
										<span class="text-[11px] text-slate-500 block mb-1">Outras empresas ligadas a este sócio:</span>
										<div class="flex flex-wrap gap-1.5">
											{#each socio.outras_empresas as emp}
												<a
													href={`/dossie/cnpj/${emp.cnpj}`}
													class="px-2 py-0.5 text-[11px] rounded bg-purple-950/60 border border-purple-800/40 text-purple-300 hover:text-white hover:border-purple-500 transition-colors"
													title={`CNPJ: ${emp.cnpj_formatado}`}
												>
													{emp.razao_social}
												</a>
											{/each}
										</div>
									</div>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>

			{#if dossie.qsa.empresas_interligadas.length > 0}
				<div class="mt-4 pt-4 border-t border-slate-700/50">
					<h3 class="text-xs font-semibold text-slate-300 mb-2">Rede de Empresas Interligadas / Coligadas:</h3>
					<div class="flex flex-wrap gap-2">
						{#each dossie.qsa.empresas_interligadas as coligada}
							<a
								href={`/dossie/cnpj/${coligada.cnpj}`}
								class="px-2.5 py-1 text-xs rounded-lg bg-slate-900 border border-slate-700 text-slate-300 hover:text-white hover:border-emerald-500 transition-colors flex items-center gap-1.5"
							>
								<span>{coligada.razao_social}</span>
								<span class="text-[10px] font-mono text-slate-500">({coligada.cnpj_formatado})</span>
							</a>
						{/each}
					</div>
				</div>
			{/if}
		</div>

		<!-- Painel 2: Gastos Parlamentares (CEAP - Câmara) -->
		<div class="bg-slate-800/80 border border-slate-700/80 rounded-2xl p-6 shadow-xl flex flex-col justify-between">
			<div class="space-y-4">
				<div class="flex items-center justify-between border-b border-slate-700/60 pb-3">
					<div class="flex items-center gap-2.5">
						<div class="w-8 h-8 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400">
							<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
							</svg>
						</div>
						<div>
							<h2 class="text-base font-bold text-white">2. Gastos Parlamentares (CEAP)</h2>
							<p class="text-xs text-slate-400">Cotas Parlamentares Faturadas na Câmara</p>
						</div>
					</div>
					<span class="text-xs px-2.5 py-1 rounded-full bg-slate-900 text-emerald-400 font-bold">
						{formatarMoeda(dossie.ceap.total_faturado)}
					</span>
				</div>

				{#if dossie.ceap.compradores.length === 0}
					<p class="text-xs text-slate-500 py-4 italic">Nenhum faturamento de cota parlamentar (CEAP) registrado para este CNPJ.</p>
				{:else}
					<div>
						<h3 class="text-xs font-semibold text-slate-300 mb-2">Principais Gabinetes Parlamentares Compradores:</h3>
						<div class="space-y-2 max-h-48 overflow-y-auto pr-1">
							{#each dossie.ceap.compradores as comp}
								<div class="p-2.5 rounded-lg bg-slate-900/60 border border-slate-700/40 flex items-center justify-between gap-2">
									<div>
										{#if comp.politico_id}
											<a
												href={`/dossie/${comp.politico_id}`}
												class="text-xs font-semibold text-emerald-300 hover:text-emerald-200 hover:underline flex items-center gap-1"
											>
												<span>{comp.parlamentar_nome}</span>
												<svg class="w-3 h-3 text-slate-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
													<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
												</svg>
											</a>
										{:else}
											<span class="text-xs font-semibold text-slate-200">{comp.parlamentar_nome}</span>
										{/if}
										<span class="text-[11px] text-slate-500 block">{comp.quantidade_notas} notas emitidas</span>
									</div>
									<span class="text-xs font-bold font-mono text-emerald-400">
										{formatarMoeda(comp.total_gasto)}
									</span>
								</div>
							{/each}
						</div>
					</div>

					<div class="pt-3 border-t border-slate-700/50">
						<h3 class="text-xs font-semibold text-slate-300 mb-2">Últimas Notas Fiscais Faturadas:</h3>
						<div class="space-y-1.5 max-h-40 overflow-y-auto pr-1 text-xs">
							{#each dossie.ceap.notas_fiscais.slice(0, 8) as nf}
								<div class="p-2 rounded bg-slate-900/40 border border-slate-800 flex items-center justify-between gap-2">
									<div class="truncate">
										<span class="text-slate-400 text-[11px]">{formatarData(nf.data_emissao)}</span>
										<span class="text-slate-200 font-medium ml-1 truncate">{nf.parlamentar_nome}</span>
										{#if nf.flag_anomalia}
											<span class="ml-1 text-[10px] px-1.5 py-0.2 rounded bg-rose-500/20 text-rose-300 border border-rose-500/40">Anomalia</span>
										{/if}
									</div>
									<div class="flex items-center gap-2 flex-shrink-0">
										<span class="font-mono text-emerald-400 font-semibold">{formatarMoeda(nf.valor_liquido)}</span>
										{#if nf.url_nota_fiscal}
											<a href={nf.url_nota_fiscal} target="_blank" rel="noreferrer" class="text-slate-400 hover:text-white" title="Ver Nota Fiscal">
												<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
													<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
												</svg>
											</a>
										{/if}
									</div>
								</div>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		</div>

		<!-- Painel 3: Contratos Públicos (PNCP) -->
		<div class="bg-slate-800/80 border border-slate-700/80 rounded-2xl p-6 shadow-xl flex flex-col justify-between">
			<div class="space-y-4">
				<div class="flex items-center justify-between border-b border-slate-700/60 pb-3">
					<div class="flex items-center gap-2.5">
						<div class="w-8 h-8 rounded-lg bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400">
							<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
							</svg>
						</div>
						<div>
							<h2 class="text-base font-bold text-white">3. Contratos Públicos (PNCP)</h2>
							<p class="text-xs text-slate-400">Licitações e Contratações Governamentais</p>
						</div>
					</div>
					<span class="text-xs px-2.5 py-1 rounded-full bg-slate-900 text-cyan-400 font-bold">
						{formatarMoeda(dossie.pncp.total_contratado)}
					</span>
				</div>

				{#if dossie.pncp.contratos.length === 0}
					<p class="text-xs text-slate-500 py-4 italic">Nenhum contrato público registrado no PNCP para este fornecedor.</p>
				{:else}
					<div>
						<h3 class="text-xs font-semibold text-slate-300 mb-2">Órgãos Contratantes:</h3>
						<div class="space-y-2 max-h-40 overflow-y-auto pr-1">
							{#each dossie.pncp.orgaos_contratantes as orgao}
								<div class="p-2.5 rounded-lg bg-slate-900/60 border border-slate-700/40 flex items-center justify-between text-xs">
									<div>
										<span class="font-medium text-slate-200 block">{orgao.orgao}</span>
										<span class="text-[11px] text-slate-500">{orgao.quantidade_contratos} termo(s) contratuais</span>
									</div>
									<span class="font-mono font-semibold text-cyan-400">{formatarMoeda(orgao.total_valor)}</span>
								</div>
							{/each}
						</div>
					</div>

					<div class="pt-3 border-t border-slate-700/50">
						<h3 class="text-xs font-semibold text-slate-300 mb-2">Contratos Recentes:</h3>
						<div class="space-y-2 max-h-48 overflow-y-auto pr-1 text-xs">
							{#each dossie.pncp.contratos as c}
								<div class="p-3 rounded-xl bg-slate-900/60 border border-slate-700/50 space-y-1">
									<div class="flex items-center justify-between">
										<span class="font-semibold text-slate-200">{c.orgao_contratante}</span>
										<span class="font-mono text-cyan-400 font-bold">{formatarMoeda(c.valor_contratado)}</span>
									</div>
									{#if c.objeto}
										<p class="text-[11px] text-slate-400 line-clamp-2">{c.objeto}</p>
									{/if}
									<div class="flex items-center gap-3 text-[10px] text-slate-500 pt-1">
										<span>Assinatura: {formatarData(c.data_assinatura)}</span>
										{#if c.data_termino}
											<span>Término: {formatarData(c.data_termino)}</span>
										{/if}
									</div>
								</div>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		</div>

		<!-- Painel 4: Vínculos Políticos e Eleitorais dos Sócios (TSE) -->
		<div class="bg-slate-800/80 border border-slate-700/80 rounded-2xl p-6 shadow-xl flex flex-col justify-between">
			<div class="space-y-4">
				<div class="flex items-center justify-between border-b border-slate-700/60 pb-3">
					<div class="flex items-center gap-2.5">
						<div class="w-8 h-8 rounded-lg bg-amber-500/10 border border-amber-500/30 flex items-center justify-center text-amber-400">
							<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z" />
							</svg>
						</div>
						<div>
							<h2 class="text-base font-bold text-white">4. Vínculos Políticos dos Sócios (TSE)</h2>
							<p class="text-xs text-slate-400">Doações de Campanha e Histórico Eleitoral</p>
						</div>
					</div>
					<span class="text-xs px-2.5 py-1 rounded-full bg-slate-900 text-amber-400 font-bold">
						{formatarMoeda(dossie.tse.total_doacoes_socios)}
					</span>
				</div>

				{#if dossie.tse.doacoes.length === 0 && dossie.tse.candidaturas_socios.length === 0}
					<p class="text-xs text-slate-500 py-4 italic">Nenhum vínculo eleitoral ou doação identificada para os sócios desta empresa no TSE.</p>
				{:else}
					{#if dossie.tse.doacoes.length > 0}
						<div>
							<h3 class="text-xs font-semibold text-slate-300 mb-2">Doações de Campanha Efetuadas por Sócios:</h3>
							<div class="space-y-2 max-h-48 overflow-y-auto pr-1">
								{#each dossie.tse.doacoes as d}
									<div class="p-3 rounded-xl bg-slate-900/60 border border-slate-700/50 space-y-1.5 text-xs">
										<div class="flex items-center justify-between">
											<span class="text-slate-400">Doador: <strong class="text-white">{d.socio_nome}</strong></span>
											<span class="font-mono text-amber-400 font-bold">{formatarMoeda(d.valor)}</span>
										</div>
										<div class="flex items-center justify-between text-[11px] text-slate-300">
											<span>Beneficiário:
												{#if d.politico_id}
													<a href={`/dossie/${d.politico_id}`} class="text-emerald-400 font-semibold hover:underline">
														{d.candidato_nome}
													</a>
												{:else}
													<strong class="text-slate-200">{d.candidato_nome}</strong>
												{/if}
												({d.partido} • {d.ano})
											</span>
											<span class="text-slate-500">{formatarData(d.data)}</span>
										</div>
									</div>
								{/each}
							</div>
						</div>
					{/if}

					{#if dossie.tse.candidaturas_socios.length > 0}
						<div class="pt-3 border-t border-slate-700/50">
							<h3 class="text-xs font-semibold text-slate-300 mb-2">Candidaturas Eleitorais de Sócios:</h3>
							<div class="space-y-2 max-h-40 overflow-y-auto pr-1 text-xs">
								{#each dossie.tse.candidaturas_socios as c}
									<div class="p-2.5 rounded-lg bg-slate-900/60 border border-slate-700/40 flex items-center justify-between">
										<div>
											<a href={`/dossie/${c.politico_id}`} class="font-semibold text-emerald-300 hover:underline">
												{c.nome_urna} ({c.socio_nome})
											</a>
											<span class="text-[11px] text-slate-400 block">{c.cargo} • {c.partido} ({c.ano} - {c.uf})</span>
										</div>
										<span class="text-[11px] font-mono text-slate-400">
											Bens: {formatarMoeda(c.total_bens)}
										</span>
									</div>
								{/each}
							</div>
						</div>
					{/if}
				{/if}
			</div>
		</div>
	</div>
</div>
