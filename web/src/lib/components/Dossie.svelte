<script lang="ts">
	import type { BemItem, CandidaturaItem, DoadorItem, DossiePolitico } from '$lib/types';

	export let dossie: DossiePolitico;

	function formatarMoeda(valor: number): string {
		return new Intl.NumberFormat('pt-BR', { style: 'currency', currency: 'BRL' }).format(valor);
	}

	function formatarData(dataStr: string | null): string {
		if (!dataStr) return 'Não informada';
		// Tenta converter YYYY-MM-DD para DD/MM/AAAA
		const partes = dataStr.split('T')[0].split('-');
		if (partes.length === 3) {
			return `${partes[2]}/${partes[1]}/${partes[0]}`;
		}
		return dataStr;
	}

	$: totalBensRecente = dossie.candidaturas.length > 0
		? dossie.candidaturas[0].total_bens_declarados
		: dossie.historico_bens.reduce((acc, b) => acc + b.valor_declarado, 0);

	$: totalDoacoesRecebidas = dossie.doadores.reduce((acc, d) => acc + d.valor, 0);
</script>

<div class="space-y-8">
	<!-- Cabeçalho do Político -->
	<div class="bg-slate-800/80 border border-slate-700/80 rounded-2xl p-6 sm:p-8 shadow-xl flex flex-col sm:flex-row items-center sm:items-start gap-6">
		<div class="relative flex-shrink-0">
			{#if dossie.foto_base64}
				<img
					src={`data:${dossie.foto_mime || 'image/jpeg'};base64,${dossie.foto_base64}`}
					alt={`Foto oficial de ${dossie.nome_urna}`}
					class="w-32 h-40 object-cover rounded-xl border-2 border-emerald-500/50 shadow-md bg-slate-900"
				/>
			{:else}
				<div class="w-32 h-40 rounded-xl border-2 border-slate-700 bg-slate-900 flex flex-col items-center justify-center text-slate-500">
					<svg class="w-12 h-12" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
					</svg>
					<span class="text-xs mt-2 font-medium">Sem foto TSE</span>
				</div>
			{/if}
		</div>

		<div class="flex-1 text-center sm:text-left">
			<div class="flex flex-wrap items-center justify-center sm:justify-start gap-2">
				<h1 class="text-2xl sm:text-3xl font-bold text-white tracking-tight">{dossie.nome_urna}</h1>
				{#if dossie.cpf_mascarado}
					<span class="px-2.5 py-0.5 text-xs bg-slate-700/80 text-slate-300 rounded-full font-mono">{dossie.cpf_mascarado}</span>
				{/if}
				{#if dossie.alertas_auxilio && dossie.alertas_auxilio.length > 0}
					<span class="px-3 py-1 text-xs bg-rose-500/20 text-rose-300 border border-rose-500/40 rounded-full font-semibold flex items-center gap-1.5 shadow-sm">
						<span class="w-2 h-2 rounded-full bg-rose-400 animate-pulse"></span>
						Alerta: Benefício Indevido ({dossie.alertas_auxilio.length})
					</span>
				{/if}
			</div>

			<p class="text-sm text-slate-400 mt-1 font-medium">{dossie.nome_completo}</p>

			<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 mt-4 text-xs text-slate-300">
				{#if dossie.ocupacao}
					<div class="bg-slate-900/60 p-2.5 rounded-lg border border-slate-700/40">
						<span class="text-slate-500 block">Ocupação Declarada:</span>
						<span class="font-medium text-slate-200">{dossie.ocupacao}</span>
					</div>
				{/if}
				{#if dossie.grau_instrucao}
					<div class="bg-slate-900/60 p-2.5 rounded-lg border border-slate-700/40">
						<span class="text-slate-500 block">Grau de Instrução:</span>
						<span class="font-medium text-slate-200">{dossie.grau_instrucao}</span>
					</div>
				{/if}
				{#if dossie.data_nascimento}
					<div class="bg-slate-900/60 p-2.5 rounded-lg border border-slate-700/40">
						<span class="text-slate-500 block">Data de Nascimento:</span>
						<span class="font-medium text-slate-200">{formatarData(dossie.data_nascimento)}</span>
					</div>
				{/if}
			</div>

			<div class="mt-5 flex flex-wrap items-center justify-center sm:justify-start gap-3">
				<a
					href={`/grafo/${dossie.id}?grau=2`}
					class="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-lg shadow-md transition-colors flex items-center gap-1.5"
				>
					<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
					</svg>
					Explorar Rede de Relacionamentos
				</a>
			</div>
		</div>
	</div>

	<!-- Cartões de Métricas -->
	<div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
		<div class="bg-slate-800/60 border border-slate-700/60 rounded-xl p-5 shadow">
			<span class="text-xs font-medium text-slate-400 uppercase tracking-wider">Patrimônio Declarado Recente</span>
			<p class="text-2xl font-bold text-emerald-400 mt-2">{formatarMoeda(totalBensRecente)}</p>
			<span class="text-xs text-slate-500 mt-1 block">TSE / DivulgaCand</span>
		</div>

		<div class="bg-slate-800/60 border border-slate-700/60 rounded-xl p-5 shadow">
			<span class="text-xs font-medium text-slate-400 uppercase tracking-wider">Total de Doações Recebidas</span>
			<p class="text-2xl font-bold text-sky-400 mt-2">{formatarMoeda(totalDoacoesRecebidas)}</p>
			<span class="text-xs text-slate-500 mt-1 block">{dossie.doadores.length} doações registradas</span>
		</div>

		<div class="bg-slate-800/60 border border-slate-700/60 rounded-xl p-5 shadow">
			<span class="text-xs font-medium text-slate-400 uppercase tracking-wider">Histórico de Eleições</span>
			<p class="text-2xl font-bold text-amber-400 mt-2">{dossie.candidaturas.length}</p>
			<span class="text-xs text-slate-500 mt-1 block">Candidaturas apuradas</span>
		</div>
	</div>

	<!-- Alerta de Benefício Indevido (Auxílio Emergencial) -->
	{#if dossie.alertas_auxilio && dossie.alertas_auxilio.length > 0}
		<div class="bg-rose-950/20 border border-rose-500/40 rounded-2xl p-6 shadow-xl relative overflow-hidden backdrop-blur-sm">
			<div class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 border-b border-rose-500/20 pb-4 mb-4">
				<div class="flex items-center gap-3">
					<div class="w-10 h-10 rounded-xl bg-rose-500/20 border border-rose-500/30 flex items-center justify-center text-rose-400 text-lg font-bold">
						⚠️
					</div>
					<div>
						<h3 class="text-lg font-bold text-rose-200 flex items-center gap-2">
							Alerta de Auditoria: Auxílio Emergencial Indevido
							<span class="px-2 py-0.5 text-xs bg-rose-500/30 text-rose-200 rounded font-mono">
								{dossie.alertas_auxilio.length} {dossie.alertas_auxilio.length === 1 ? 'registro' : 'registros'}
							</span>
						</h3>
						<p class="text-xs text-rose-300/80 mt-0.5">
							Identificado cruzamento determinístico com incompatibilidade legal (Lei 13.982/2020 / Base CGU).
						</p>
					</div>
				</div>
				<div class="bg-slate-900/80 px-3.5 py-1.5 rounded-lg border border-rose-500/30 text-right">
					<span class="text-[10px] text-slate-400 block uppercase font-medium">Total de Benefício Flagrado</span>
					<span class="font-mono text-base font-bold text-rose-400">
						{formatarMoeda(dossie.alertas_auxilio.reduce((acc, a) => acc + a.valor_recebido, 0))}
					</span>
				</div>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
				{#each dossie.alertas_auxilio as alerta}
					<div class="bg-slate-900/90 border border-rose-500/30 rounded-xl p-4 flex flex-col justify-between hover:border-rose-500/50 transition-colors">
						<div>
							<div class="flex items-center justify-between gap-2 flex-wrap">
								<span class="px-2.5 py-0.5 text-xs font-bold rounded bg-rose-500/20 text-rose-300 border border-rose-500/40">
									{alerta.motivo.replace(/_/g, ' ')}
								</span>
								{#if alerta.mes_disponibilizacao}
									<span class="text-xs text-slate-400 font-mono">
										Mês: {alerta.mes_disponibilizacao} {alerta.parcela ? `(${alerta.parcela})` : ''}
									</span>
								{/if}
							</div>
							<p class="text-xs text-slate-300 mt-2.5 leading-relaxed">
								{alerta.detalhes || 'Recebimento de benefício com mandato eletivo vigente ou patrimônio superior a R$ 300.000,00.'}
							</p>
						</div>

						<div class="mt-4 pt-3 border-t border-slate-800 flex items-center justify-between text-xs">
							<span class="text-slate-400">
								{#if alerta.cargo_ou_mandato}
									Mandato: <strong class="text-slate-200">{alerta.cargo_ou_mandato}</strong>
								{:else if alerta.total_bens}
									Bens Declarados: <strong class="text-amber-400">{formatarMoeda(alerta.total_bens)}</strong>
								{/if}
							</span>
							<span class="font-mono font-bold text-rose-400 text-sm">
								{formatarMoeda(alerta.valor_recebido)}
							</span>
						</div>
					</div>
				{/each}
			</div>
		</div>
	{/if}

	<!-- Candidaturas -->
	<div class="bg-slate-800/60 border border-slate-700/60 rounded-xl p-6 shadow">
		<h3 class="text-lg font-bold text-white mb-4 flex items-center gap-2">
			<span>Candidaturas Registradas</span>
		</h3>
		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
			{#each dossie.candidaturas as cand}
				<div class="bg-slate-900/70 border border-slate-700/50 rounded-lg p-4">
					<div class="flex items-center justify-between">
						<span class="text-xs font-bold text-emerald-400">{cand.ano_eleicao}</span>
						<span class="text-xs bg-slate-800 text-slate-300 px-2 py-0.5 rounded font-semibold">{cand.sigla_partido}</span>
					</div>
					<h4 class="font-semibold text-slate-100 text-base mt-2">{cand.cargo}</h4>
					<p class="text-xs text-slate-400 mt-0.5">
						{cand.municipio ? `${cand.municipio} - ` : ''}{cand.uf}
						{#if cand.numero_urna}
							• Nº {cand.numero_urna}
						{/if}
					</p>
					{#if cand.situacao_totalizacao}
						<span class="mt-2.5 inline-block text-xs font-medium px-2 py-0.5 rounded bg-slate-800/90 text-slate-300 border border-slate-700">
							{cand.situacao_totalizacao}
						</span>
					{/if}
				</div>
			{/each}
		</div>
	</div>

	<!-- Histórico de Bens Declarados -->
	<div class="bg-slate-800/60 border border-slate-700/60 rounded-xl p-6 shadow overflow-hidden">
		<h3 class="text-lg font-bold text-white mb-4">Histórico de Bens Declarados</h3>
		{#if dossie.historico_bens.length > 0}
			<div class="overflow-x-auto">
				<table class="w-full text-left text-sm text-slate-300">
					<thead class="text-xs uppercase bg-slate-900/80 text-slate-400 border-b border-slate-700">
						<tr>
							<th class="py-3 px-4">Ano</th>
							<th class="py-3 px-4">Tipo do Bem</th>
							<th class="py-3 px-4">Descrição</th>
							<th class="py-3 px-4 text-right">Valor Declarado</th>
						</tr>
					</thead>
					<tbody class="divide-y divide-slate-700/50">
						{#each dossie.historico_bens as bem}
							<tr class="hover:bg-slate-750/50 transition-colors">
								<td class="py-3 px-4 font-semibold text-emerald-400">{bem.ano_eleicao}</td>
								<td class="py-3 px-4 text-slate-200">{bem.tipo_bem || 'OUTROS'}</td>
								<td class="py-3 px-4 text-xs text-slate-400">{bem.descricao || '-'}</td>
								<td class="py-3 px-4 text-right font-mono text-slate-100">{formatarMoeda(bem.valor_declarado)}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p class="text-sm text-slate-400">Nenhum bem patrimonial registrado no TSE.</p>
		{/if}
	</div>

	<!-- Tabela de Doadores -->
	<div class="bg-slate-800/60 border border-slate-700/60 rounded-xl p-6 shadow overflow-hidden">
		<h3 class="text-lg font-bold text-white mb-4">Principais Doadores de Campanha</h3>
		{#if dossie.doadores.length > 0}
			<div class="overflow-x-auto">
				<table class="w-full text-left text-sm text-slate-300">
					<thead class="text-xs uppercase bg-slate-900/80 text-slate-400 border-b border-slate-700">
						<tr>
							<th class="py-3 px-4">Doador</th>
							<th class="py-3 px-4">Documento</th>
							<th class="py-3 px-4">Ano</th>
							<th class="py-3 px-4">Data</th>
							<th class="py-3 px-4 text-right">Valor</th>
							<th class="py-3 px-4 text-center">Auditoria</th>
						</tr>
					</thead>
					<tbody class="divide-y divide-slate-700/50">
						{#each dossie.doadores as doador}
							<tr class="hover:bg-slate-750/50 transition-colors">
								<td class="py-3 px-4 font-medium text-slate-100">{doador.doador_nome}</td>
								<td class="py-3 px-4 font-mono text-xs text-slate-400">{doador.doador_cpf_cnpj}</td>
								<td class="py-3 px-4 text-xs text-slate-400">{doador.ano_eleicao}</td>
								<td class="py-3 px-4 text-xs text-slate-400">{formatarData(doador.data_receita)}</td>
								<td class="py-3 px-4 text-right font-mono text-sky-400 font-semibold">{formatarMoeda(doador.valor)}</td>
								<td class="py-3 px-4 text-center">
									<a
										href={`/investigar/${doador.id}`}
										class="px-2.5 py-1 text-xs bg-slate-700 hover:bg-slate-600 text-slate-200 rounded font-medium transition-colors"
									>
										Investigar
									</a>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<p class="text-sm text-slate-400">Nenhum doador de campanha registrado para esta candidatura.</p>
		{/if}
	</div>
</div>
