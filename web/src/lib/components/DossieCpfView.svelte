<script lang="ts">
	import type { DossieCpf } from '$lib/types';

	export let dossie: DossieCpf;

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

	$: totalDoacoesFeitas = dossie.doacoes_eleitorais.reduce((acc, d) => acc + d.valor, 0);
</script>

<div class="space-y-8">
	<!-- Cabeçalho do Dossiê CPF -->
	<div class="bg-slate-800/80 border border-slate-700/80 rounded-2xl p-6 sm:p-8 shadow-xl">
		<div class="flex flex-col md:flex-row md:items-start justify-between gap-6">
			<div class="space-y-3">
				<div class="flex flex-wrap items-center gap-2.5">
					<span class="px-2.5 py-1 text-xs font-bold rounded-lg bg-purple-500/10 border border-purple-500/30 text-purple-400">
						PESSOA FÍSICA (CPF / SÓCIO)
					</span>
					<span class="px-3 py-1 text-xs font-mono rounded-lg bg-slate-900 border border-slate-700 text-slate-300">
						{dossie.cpf_mascarado}
					</span>
					{#if dossie.registros_profissionais.length > 0}
						{#each dossie.registros_profissionais as reg}
							<span class="px-2.5 py-1 text-xs rounded-lg bg-indigo-500/10 border border-indigo-500/30 text-indigo-400 font-mono">
								{reg.orgao} {reg.numero}/{reg.uf} ({reg.situacao})
							</span>
						{/each}
					{/if}
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
					{dossie.nome}
				</h1>
				<p class="text-xs sm:text-sm text-slate-400">
					Dossiê analítico integrado de pessoa física: participações societárias (QSA), contratos públicos (PNCP), cotas parlamentares (CEAP) e registros eleitorais (TSE).
				</p>
			</div>

			<div class="flex items-center gap-2 self-start flex-shrink-0">
				<a
					href={`/grafo/socio_${encodeURIComponent(dossie.nome)}?grau=2`}
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
				<span class="text-xs text-slate-400 block font-medium">Empresas como Sócio</span>
				<span class="text-lg sm:text-xl font-bold text-purple-400 mt-1 block">
					{dossie.empresas_socio.length}
				</span>
				<span class="text-[11px] text-slate-500">participações registradas no QSA</span>
			</div>

			<div class="bg-slate-900/60 p-4 rounded-xl border border-slate-700/40">
				<span class="text-xs text-slate-400 block font-medium">Faturamento CEAP (Empresas)</span>
				<span class="text-lg sm:text-xl font-bold text-emerald-400 mt-1 block">
					{formatarMoeda(dossie.total_faturado_empresas_ceap)}
				</span>
				<span class="text-[11px] text-slate-500">{dossie.notas_ceap_empresas.length} notas faturadas</span>
			</div>

			<div class="bg-slate-900/60 p-4 rounded-xl border border-slate-700/40">
				<span class="text-xs text-slate-400 block font-medium">Contratos PNCP (Empresas)</span>
				<span class="text-lg sm:text-xl font-bold text-cyan-400 mt-1 block">
					{formatarMoeda(dossie.total_contratado_empresas_pncp)}
				</span>
				<span class="text-[11px] text-slate-500">{dossie.contratos_pncp_empresas.length} contratos públicos</span>
			</div>

			<div class="bg-slate-900/60 p-4 rounded-xl border border-slate-700/40">
				<span class="text-xs text-slate-400 block font-medium">Doações de Campanha (TSE)</span>
				<span class="text-lg sm:text-xl font-bold text-amber-400 mt-1 block">
					{formatarMoeda(totalDoacoesFeitas)}
				</span>
				<span class="text-[11px] text-slate-500">{dossie.doacoes_eleitorais.length} doações registradas</span>
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
					<h2 class="text-base font-bold text-white">Alertas e Cruzamentos Críticos Identificados</h2>
					<p class="text-xs text-slate-400">Incompatibilidades da Lei 8.906/94, triangulações eleitorais e benefícios indevidos</p>
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
		<!-- Painel 1: Quadro Societário (Empresas onde figura como sócio) -->
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
							<h2 class="text-base font-bold text-white">1. Participações Societárias (QSA)</h2>
							<p class="text-xs text-slate-400">Pessoas Jurídicas vinculadas como sócio ou administrador</p>
						</div>
					</div>
					<span class="text-xs px-2.5 py-1 rounded-full bg-slate-900 text-purple-300 font-mono">
						{dossie.empresas_socio.length} empresas
					</span>
				</div>

				{#if dossie.empresas_socio.length === 0}
					<p class="text-xs text-slate-500 py-4 italic">Nenhuma empresa associada a este CPF no QSA.</p>
				{:else}
					<div class="space-y-3 max-h-96 overflow-y-auto pr-1">
						{#each dossie.empresas_socio as emp}
							<div class="p-3.5 rounded-xl bg-slate-900/60 border border-slate-700/50 space-y-2">
								<div class="flex items-center justify-between gap-2">
									<a
										href={`/dossie/cnpj/${emp.cnpj}`}
										class="text-sm font-semibold text-purple-300 hover:text-purple-200 hover:underline flex items-center gap-1.5"
									>
										<span>{emp.razao_social}</span>
										<svg class="w-3.5 h-3.5 text-slate-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
											<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
										</svg>
									</a>
									<span class="text-[11px] font-mono px-2 py-0.5 rounded bg-slate-800 text-slate-400">
										{emp.cnpj_formatado}
									</span>
								</div>
								{#if emp.qualificacao}
									<p class="text-xs text-slate-400">Qualificação: <strong class="text-slate-300">{emp.qualificacao}</strong></p>
								{/if}

								<div class="flex items-center gap-4 pt-2 border-t border-slate-800/80 text-xs">
									{#if emp.total_ceap > 0}
										<div>
											<span class="text-slate-500 text-[11px]">CEAP: </span>
											<span class="font-mono font-semibold text-emerald-400">{formatarMoeda(emp.total_ceap)}</span>
										</div>
									{/if}
									{#if emp.total_pncp > 0}
										<div>
											<span class="text-slate-500 text-[11px]">PNCP: </span>
											<span class="font-mono font-semibold text-cyan-400">{formatarMoeda(emp.total_pncp)}</span>
										</div>
									{/if}
									{#if emp.total_ceap === 0 && emp.total_pncp === 0}
										<span class="text-[11px] text-slate-500">Sem faturamento CEAP/PNCP identificado</span>
									{/if}
								</div>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		</div>

		<!-- Painel 2: Gastos Parlamentares das Empresas (CEAP) -->
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
							<h2 class="text-base font-bold text-white">2. Cotas Parlamentares (CEAP)</h2>
							<p class="text-xs text-slate-400">Faturamento das empresas do sócio na Câmara</p>
						</div>
					</div>
					<span class="text-xs px-2.5 py-1 rounded-full bg-slate-900 text-emerald-400 font-bold">
						{formatarMoeda(dossie.total_faturado_empresas_ceap)}
					</span>
				</div>

				{#if dossie.notas_ceap_empresas.length === 0}
					<p class="text-xs text-slate-500 py-4 italic">Nenhuma despesa parlamentar (CEAP) faturada para as empresas deste sócio.</p>
				{:else}
					<div class="space-y-2 max-h-96 overflow-y-auto pr-1 text-xs">
						{#each dossie.notas_ceap_empresas as nf}
							<div class="p-3 rounded-xl bg-slate-900/60 border border-slate-700/50 space-y-1.5">
								<div class="flex items-center justify-between gap-2">
									<div>
										{#if nf.politico_id}
											<a
												href={`/dossie/${nf.politico_id}`}
												class="font-semibold text-emerald-300 hover:text-emerald-200 hover:underline flex items-center gap-1"
											>
												<span>{nf.parlamentar_nome}</span>
												<svg class="w-3 h-3 text-slate-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
													<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
												</svg>
											</a>
										{:else}
											<span class="font-semibold text-slate-200">{nf.parlamentar_nome}</span>
										{/if}
										<span class="text-[11px] text-slate-400 block">{formatarData(nf.data_emissao)} • {nf.categoria_despesa}</span>
									</div>
									<div class="text-right">
										<span class="font-mono text-emerald-400 font-bold block">{formatarMoeda(nf.valor_liquido)}</span>
										{#if nf.flag_anomalia}
											<span class="text-[10px] px-1.5 py-0.5 rounded bg-rose-500/20 text-rose-300 border border-rose-500/40">Anomalia</span>
										{/if}
									</div>
								</div>
								{#if nf.url_nota_fiscal}
									<div class="pt-1 border-t border-slate-800 text-[11px]">
										<a href={nf.url_nota_fiscal} target="_blank" rel="noreferrer" class="text-cyan-400 hover:underline flex items-center gap-1">
											<span>Visualizar Nota Fiscal Original</span>
											<svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
												<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
											</svg>
										</a>
									</div>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>
		</div>

		<!-- Painel 3: Contratos Públicos das Empresas (PNCP) -->
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
							<p class="text-xs text-slate-400">Contratos firmados pelas empresas com o Poder Público</p>
						</div>
					</div>
					<span class="text-xs px-2.5 py-1 rounded-full bg-slate-900 text-cyan-400 font-bold">
						{formatarMoeda(dossie.total_contratado_empresas_pncp)}
					</span>
				</div>

				{#if dossie.contratos_pncp_empresas.length === 0}
					<p class="text-xs text-slate-500 py-4 italic">Nenhum contrato no PNCP registrado para as empresas deste sócio.</p>
				{:else}
					<div class="space-y-2 max-h-96 overflow-y-auto pr-1 text-xs">
						{#each dossie.contratos_pncp_empresas as c}
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
				{/if}
			</div>
		</div>

		<!-- Painel 4: Vínculos Políticos e Doações (TSE) -->
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
							<h2 class="text-base font-bold text-white">4. Vínculos Políticos e Doações (TSE)</h2>
							<p class="text-xs text-slate-400">Doações realizadas e candidaturas eleitorais</p>
						</div>
					</div>
					<span class="text-xs px-2.5 py-1 rounded-full bg-slate-900 text-amber-400 font-bold">
						{formatarMoeda(totalDoacoesFeitas)}
					</span>
				</div>

				{#if dossie.doacoes_eleitorais.length === 0 && dossie.candidaturas.length === 0}
					<p class="text-xs text-slate-500 py-4 italic">Nenhuma doação ou candidatura eleitoral registrada no TSE.</p>
				{:else}
					{#if dossie.doacoes_eleitorais.length > 0}
						<div>
							<h3 class="text-xs font-semibold text-slate-300 mb-2">Doações Financeiras para Campanhas:</h3>
							<div class="space-y-2 max-h-48 overflow-y-auto pr-1">
								{#each dossie.doacoes_eleitorais as d}
									<div class="p-3 rounded-xl bg-slate-900/60 border border-slate-700/50 space-y-1.5 text-xs">
										<div class="flex items-center justify-between">
											<div class="flex items-center gap-1.5">
												<span class="text-slate-400">Beneficiário:</span>
												{#if d.politico_id}
													<a href={`/dossie/${d.politico_id}`} class="text-emerald-400 font-semibold hover:underline">
														{d.politico_nome}
													</a>
												{:else}
													<strong class="text-slate-200">{d.politico_nome}</strong>
												{/if}
											</div>
											<span class="font-mono text-amber-400 font-bold">{formatarMoeda(d.valor)}</span>
										</div>
										<div class="flex items-center justify-between text-[11px] text-slate-400">
											<span>{d.cargo} • {d.partido} ({d.ano_eleicao})</span>
											<span>{formatarData(d.data_receita)}</span>
										</div>
									</div>
								{/each}
							</div>
						</div>
					{/if}

					{#if dossie.candidaturas.length > 0}
						<div class="pt-3 border-t border-slate-700/50">
							<h3 class="text-xs font-semibold text-slate-300 mb-2">Histórico de Candidaturas Eleitorais:</h3>
							<div class="space-y-2 max-h-40 overflow-y-auto pr-1 text-xs">
								{#each dossie.candidaturas as c}
									<div class="p-2.5 rounded-lg bg-slate-900/60 border border-slate-700/40 flex items-center justify-between">
										<div>
											<a href={`/dossie/${c.politico_id}`} class="font-semibold text-emerald-300 hover:underline">
												{c.nome_urna}
											</a>
											<span class="text-[11px] text-slate-400 block">{c.cargo} • {c.partido} ({c.ano_eleicao} - {c.uf})</span>
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

	<!-- Painel Adicional: Benefícios Emergenciais / Auxílios se houver -->
	{#if dossie.beneficios_emergenciais.length > 0}
		<div class="bg-slate-800/80 border border-slate-700/80 rounded-2xl p-6 shadow-xl space-y-4">
			<div class="flex items-center gap-3">
				<div class="w-8 h-8 rounded-lg bg-rose-500/10 border border-rose-500/30 flex items-center justify-center text-rose-400">
					<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
					</svg>
				</div>
				<div>
					<h2 class="text-base font-bold text-white">Benefícios e Auxílios do Governo Federal</h2>
					<p class="text-xs text-slate-400">Auxílio Emergencial / Benefícios Sociais cruzados com participações societárias</p>
				</div>
			</div>

			<div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-3">
				{#each dossie.beneficios_emergenciais as b}
					<div class="p-3 rounded-xl bg-slate-900/60 border border-slate-700/50 text-xs space-y-1">
						<div class="flex items-center justify-between">
							<span class="text-slate-400 font-mono">{b.mes}</span>
							<span class="font-mono font-bold text-rose-400">{formatarMoeda(b.valor)}</span>
						</div>
						<div class="text-[11px] text-slate-400">
							{b.parcela || 'Parcela não informada'} • {b.enquadramento || 'Geral'}
						</div>
					</div>
				{/each}
			</div>
		</div>
	{/if}
</div>
