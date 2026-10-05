<script lang="ts">
	import { onMount } from 'svelte';
	import type { AlertaItem } from '$lib/types';

	let alertas: AlertaItem[] = [];
	let loading = true;
	let total = 0;

	// Filtros
	let filtroAno = '';
	let filtroMunicipio = '';
	let filtroSeveridade = '';
	let filtroTipo = '';

	export async function carregarAlertas() {
		loading = true;
		try {
			const params = new URLSearchParams();
			if (filtroAno) params.append('ano', filtroAno);
			if (filtroMunicipio.trim()) params.append('municipio', filtroMunicipio.trim());
			if (filtroSeveridade) params.append('severidade', filtroSeveridade);
			if (filtroTipo) params.append('tipo', filtroTipo);

			const res = await fetch(`/api/v1/auditoria/alertas?${params.toString()}`);
			if (res.ok) {
				const data = await res.json();
				alertas = data.alertas || [];
				total = data.total || 0;
			}
		} catch (err) {
			console.error('Erro ao buscar alertas:', err);
		} finally {
			loading = false;
		}
	}

	function formatarMoeda(valor: number | null): string {
		if (valor === null || valor === undefined) return '-';
		return new Intl.NumberFormat('pt-BR', { style: 'currency', currency: 'BRL' }).format(valor);
	}

	function corSeveridade(sev: string): { bg: string; text: string; border: string } {
		const s = sev.toUpperCase();
		if (s === 'CRITICA' || s === 'CRÍTICA') {
			return { bg: 'bg-rose-500/10', text: 'text-rose-400', border: 'border-rose-500/30' };
		}
		if (s === 'ALTA') {
			return { bg: 'bg-amber-500/10', text: 'text-amber-400', border: 'border-amber-500/30' };
		}
		if (s === 'MEDIA' || s === 'MÉDIA') {
			return { bg: 'bg-yellow-500/10', text: 'text-yellow-400', border: 'border-yellow-500/30' };
		}
		return { bg: 'bg-slate-700/30', text: 'text-slate-300', border: 'border-slate-700' };
	}

	onMount(() => {
		carregarAlertas();
	});
</script>

<div class="space-y-6">
	<!-- Barra de Filtros -->
	<div class="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg">
		<h3 class="text-sm font-semibold text-slate-300 uppercase tracking-wider mb-4 flex items-center gap-2">
			<svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
			</svg>
			Filtros do Motor de Auditoria
		</h3>

		<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
			<div>
				<label for="filtro-ano" class="block text-xs font-medium text-slate-400 mb-1">Ano da Eleição/Despesa</label>
				<select
					id="filtro-ano"
					bind:value={filtroAno}
					on:change={carregarAlertas}
					class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-200 focus:outline-none focus:ring-1 focus:ring-emerald-500"
				>
					<option value="">Todos os anos</option>
					<option value="2024">2024</option>
					<option value="2023">2023</option>
					<option value="2022">2022</option>
					<option value="2020">2020</option>
				</select>
			</div>

			<div>
				<label for="filtro-municipio" class="block text-xs font-medium text-slate-400 mb-1">Município / UF</label>
				<input
					id="filtro-municipio"
					type="text"
					bind:value={filtroMunicipio}
					on:input={carregarAlertas}
					placeholder="Ex: São Paulo, Campinas..."
					class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:ring-1 focus:ring-emerald-500"
				/>
			</div>

			<div>
				<label for="filtro-severidade" class="block text-xs font-medium text-slate-400 mb-1">Severidade</label>
				<select
					id="filtro-severidade"
					bind:value={filtroSeveridade}
					on:change={carregarAlertas}
					class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-200 focus:outline-none focus:ring-1 focus:ring-emerald-500"
				>
					<option value="">Todas as severidades</option>
					<option value="CRITICA">Crítica</option>
					<option value="ALTA">Alta</option>
					<option value="MEDIA">Média</option>
					<option value="BAIXA">Baixa</option>
				</select>
			</div>

			<div>
				<label for="filtro-tipo" class="block text-xs font-medium text-slate-400 mb-1">Tipo de Anomalia</label>
				<select
					id="filtro-tipo"
					bind:value={filtroTipo}
					on:change={carregarAlertas}
					class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-sm text-slate-200 focus:outline-none focus:ring-1 focus:ring-emerald-500"
				>
					<option value="">Todos os tipos</option>
					<option value="COMBUSTIVEL">Combustível (&gt;80L Fiat Uno)</option>
					<option value="CONFLITO_OAB">Conflito OAB (Art. 28)</option>
					<option value="TRIANGULACAO">Triangulação (&lt;180 dias)</option>
					<option value="FORNECEDOR_HUB">Fornecedor Hub (&gt;70%)</option>
					<option value="FANTASMA">Fornecedor Inapto/Recente</option>
					<option value="UBIQUIDADE">Inconsistência Geotemporal</option>
					<option value="AUXILIO_EMERGENCIAL">Auxílio Emergencial Indevido</option>
				</select>
			</div>
		</div>
	</div>

	<!-- Lista de Alertas -->
	<div>
		<div class="flex items-center justify-between mb-4">
			<h2 class="text-lg font-bold text-white flex items-center gap-2">
				<span>Ranking de Irregularidades Detectadas</span>
				<span class="px-2 py-0.5 text-xs bg-slate-800 text-slate-300 rounded-full font-normal">
					{total} ocorrência{total === 1 ? '' : 's'}
				</span>
			</h2>
		</div>

		{#if loading}
			<div class="py-12 flex justify-center items-center">
				<div class="w-8 h-8 border-2 border-emerald-500 border-t-transparent rounded-full animate-spin"></div>
			</div>
		{:else if alertas.length > 0}
			<div class="space-y-4">
				{#each alertas as alerta}
					{@const estilo = corSeveridade(alerta.severidade)}
					<div class={`bg-slate-800/80 border ${estilo.border} rounded-xl p-5 shadow-lg transition-all hover:border-slate-600`}>
						<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
							<div class="flex items-center gap-3">
								<span class={`px-2.5 py-1 text-xs font-bold rounded-lg border ${estilo.bg} ${estilo.text} ${estilo.border}`}>
									{alerta.severidade}
								</span>
								<span class="px-2 py-0.5 text-xs font-semibold rounded bg-slate-900 text-slate-300 border border-slate-700">
									{alerta.tipo}
								</span>
								{#if alerta.fonte_dado}
									<span class="text-xs text-slate-500 font-mono">Fonte: {alerta.fonte_dado}</span>
								{/if}
							</div>

							{#if alerta.valor_envolvido}
								<div class="text-right">
									<span class="text-xs text-slate-400 block">Valor Envolvido:</span>
									<span class="text-base font-bold font-mono text-emerald-400">{formatarMoeda(alerta.valor_envolvido)}</span>
								</div>
							{/if}
						</div>

						<h3 class="text-base font-bold text-slate-100 mt-3">{alerta.titulo}</h3>
						<p class="text-sm text-slate-300 mt-1 leading-relaxed">{alerta.descricao}</p>

						<div class="mt-4 pt-3 border-t border-slate-700/50 flex flex-wrap items-center justify-between text-xs text-slate-400 gap-2">
							<div>
								Alvo: <span class="font-semibold text-slate-200">{alerta.alvo_nome}</span>
								{#if alerta.alvo_documento}
									<span class="font-mono text-slate-400 ml-1">({alerta.alvo_documento})</span>
								{/if}
								{#if alerta.municipio || alerta.uf}
									• <span class="text-slate-300">{alerta.municipio || ''} {alerta.uf ? `(${alerta.uf})` : ''}</span>
								{/if}
							</div>

							{#if alerta.ano}
								<span class="text-slate-500">Exercício: {alerta.ano}</span>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		{:else}
			<div class="p-12 text-center bg-slate-800/40 rounded-xl border border-slate-800">
				<p class="text-slate-400 text-sm">Nenhum alerta encontrado com os filtros selecionados.</p>
			</div>
		{/if}
	</div>
</div>
