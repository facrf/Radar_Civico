<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { AlertaItem } from '$lib/types';

	let alertas: AlertaItem[] = [];
	let loading = true;
	let executandoAuditoria = false;
	let erroAuditoria: string | null = null;
	let request: AbortController | null = null;
	let total = 0;
	let alertaSelecionado: AlertaItem | null = null;

	// Filtros
	let filtroAno = '';
	let filtroMunicipio = '';
	let filtroSeveridade = '';
	let filtroTipo = '';
	let debounceMunicipio: ReturnType<typeof setTimeout>;

	export async function carregarAlertas() {
		request?.abort();
		const current = new AbortController();
		request = current;
		loading = true;
		try {
			const params = new URLSearchParams();
			if (filtroAno) params.append('ano', filtroAno);
			if (filtroMunicipio.trim()) params.append('municipio', filtroMunicipio.trim());
			if (filtroSeveridade) params.append('severidade', filtroSeveridade);
			if (filtroTipo) params.append('tipo', filtroTipo);

			const res = await fetch(`/api/v1/auditoria/alertas?${params.toString()}`, { signal: current.signal });
			if (res.ok) {
				const data = await res.json();
				if (request !== current) return;
				alertas = data.alertas || [];
				total = data.total || 0;
				// Notifica layout para atualizar badge no menu superior
				if (typeof window !== 'undefined') {
					window.dispatchEvent(new CustomEvent('radar-alertas-updated'));
				}
			}
		} catch (err) {
			if (!current.signal.aborted) console.error('Erro ao buscar alertas:', err);
		} finally {
			if (request === current) loading = false;
		}
	}

	async function executarAuditoria() {
		if (executandoAuditoria) return;
		executandoAuditoria = true;
		erroAuditoria = null;
		try {
			const res = await fetch('/api/v1/auditoria/sincronizar', { method: 'POST' });
			if (!res.ok) {
				const data = await res.json();
				throw new Error(data.mensagem || 'Falha ao executar auditoria');
			}
			await carregarAlertas();
		} catch (e) {
			erroAuditoria = e instanceof Error ? e.message : 'Falha ao executar auditoria';
		} finally {
			executandoAuditoria = false;
		}
	}

	function linkSeguro(value: unknown): string | null {
		if (typeof value !== 'string') return null;
		try {
			const url = new URL(value);
			return ['http:', 'https:'].includes(url.protocol) ? url.href : null;
		} catch { return null; }
	}

	function handleMunicipioInput() {
		clearTimeout(debounceMunicipio);
		debounceMunicipio = setTimeout(() => {
			carregarAlertas();
		}, 350);
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

	function fecharModal() {
		alertaSelecionado = null;
	}

	onMount(() => {
		carregarAlertas();
	});
	onDestroy(() => { request?.abort(); clearTimeout(debounceMunicipio); });
</script>

<div class="space-y-6">
	<div class="flex flex-wrap items-center gap-3">
		<button type="button" on:click={executarAuditoria} disabled={executandoAuditoria} class="px-4 py-2 rounded-lg bg-emerald-600 text-sm text-white disabled:opacity-50">
			{executandoAuditoria ? 'Executando auditoria…' : 'Executar auditoria'}
		</button>
		<p class="text-xs text-slate-400">Consultar ou filtrar a lista não executa uma nova auditoria.</p>
	</div>
	{#if erroAuditoria}<p role="alert" class="text-sm text-rose-400">{erroAuditoria}</p>{/if}
	<!-- Barra de Filtros -->
	<div class="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg">
		<div class="flex items-center justify-between mb-4">
			<h3 class="text-sm font-semibold text-slate-300 uppercase tracking-wider flex items-center gap-2">
				<svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
				</svg>
				Filtros do Motor de Auditoria
			</h3>

			<button
				type="button"
				on:click={carregarAlertas}
				class="text-xs text-slate-400 hover:text-emerald-400 flex items-center gap-1 transition-colors"
				title="Recarregar alertas"
			>
				<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
				</svg>
				Atualizar
			</button>
		</div>

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
					on:input={handleMunicipioInput}
					placeholder="Ex: São Paulo, Brasília..."
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
					<option value="COMBUSTIVEL">Combustível (&gt;80L Tanque)</option>
					<option value="CONFLITO_OAB">Conflito OAB (Art. 28)</option>
					<option value="TRIANGULACAO">Triangulação (&lt;180 dias)</option>
					<option value="FORNECEDOR_HUB">Fornecedor Hub (&gt;70%)</option>
					<option value="FANTASMA">Fornecedor Inapto/Recente</option>
					<option value="EVOLUCAO_PATRIMONIAL">Evolução Patrimonial Abrupta</option>
					<option value="DOADOR_INCOMPATIVEL">Doador Beneficiário Social</option>
					<option value="CONLUIO_LICITACAO">Sócios Comuns em Licitação</option>
					<option value="CAPITAL_DESPROPORCIONAL">Capital Social Ínfimo vs Faturamento</option>
					<option value="EMPRESA_RECEM_CRIADA">Empresa Recém-Criada (&lt;180 dias)</option>
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
				<span class="px-2.5 py-0.5 text-xs bg-slate-800 text-slate-300 rounded-full font-normal border border-slate-700">
					{total} ocorrência{total === 1 ? '' : 's'}
				</span>
			</h2>
			<span class="text-xs text-slate-500">Clique em um alerta para ver os detalhes da irregularidade</span>
		</div>

		{#if loading}
			<div class="py-12 flex justify-center items-center">
				<div class="w-8 h-8 border-2 border-emerald-500 border-t-transparent rounded-full animate-spin"></div>
			</div>
		{:else if alertas.length > 0}
			<div class="space-y-4">
				{#each alertas as alerta}
					{@const estilo = corSeveridade(alerta.severidade)}
					<div
						class={`bg-slate-800/80 border ${estilo.border} rounded-xl p-5 shadow-lg transition-all hover:border-slate-500 cursor-pointer group`}
						on:click={() => (alertaSelecionado = alerta)}
						on:keydown={(e) => e.key === 'Enter' && (alertaSelecionado = alerta)}
						role="button"
						tabindex="0"
					>
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

						<h3 class="text-base font-bold text-slate-100 mt-3 group-hover:text-emerald-400 transition-colors">
							{alerta.titulo}
						</h3>
						<p class="text-sm text-slate-300 mt-1 leading-relaxed">{alerta.descricao}</p>
						{#if alerta.detalhes?.volume_estimado === true}
							<p class="mt-2 text-xs text-amber-300 font-medium">Volume estimado • conferir nota fiscal</p>
						{/if}
						<div class="mt-2 flex gap-4 text-xs text-emerald-400">
							{#if linkSeguro(alerta.detalhes?.fonte_primaria_url)}<a href={linkSeguro(alerta.detalhes?.fonte_primaria_url) || undefined} target="_blank" rel="noopener noreferrer" on:click|stopPropagation>Nota fiscal na fonte pública</a>{/if}
							{#if linkSeguro(alerta.detalhes?.referencia_url)}<a href={linkSeguro(alerta.detalhes?.referencia_url) || undefined} target="_blank" rel="noopener noreferrer" on:click|stopPropagation>Fonte do preço de referência</a>{/if}
						</div>

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

							<div class="flex items-center gap-3">
								{#if alerta.ano}
									<span class="text-slate-500">Exercício: {alerta.ano}</span>
								{/if}
								<span class="text-emerald-400 font-medium group-hover:underline flex items-center gap-1">
									Ver Detalhes
									<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
										<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
									</svg>
								</span>
							</div>
						</div>
					</div>
				{/each}
			</div>
		{:else}
			<div class="p-10 text-center bg-slate-800/40 rounded-xl border border-slate-800 space-y-3">
				<div class="w-12 h-12 mx-auto rounded-full bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400">
					<svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
					</svg>
				</div>
				<h3 class="text-base font-semibold text-slate-200">Nenhum alerta encontrado para estes filtros</h3>
				<p class="text-slate-400 text-xs sm:text-sm max-w-md mx-auto leading-relaxed">
					O motor de auditoria analisa notas da CEAP (volume de combustível ajustável nas Configurações), cruzamentos com doadores de campanha, contratos do PNCP, auxílio emergencial e incompatibilidade com a advocacia (Art. 28 OAB).
				</p>
				<button
					type="button"
					on:click={executarAuditoria}
					disabled={executandoAuditoria}
					class="mt-2 px-4 py-2 text-xs font-semibold bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 rounded-lg transition-colors inline-flex items-center gap-1.5"
				>
					<svg class="w-3.5 h-3.5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
					</svg>
					Sincronizar e Executar Auditoria
				</button>
			</div>
		{/if}
	</div>
</div>

<!-- Modal de Detalhes da Irregularidade -->
{#if alertaSelecionado}
	{@const modalEstilo = corSeveridade(alertaSelecionado.severidade)}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center p-4"
		role="dialog"
		aria-modal="true"
		aria-labelledby="modal-titulo"
	>
		<!-- Backdrop overlay -->
		<div
			class="fixed inset-0 bg-black/70 backdrop-blur-sm"
			on:click={fecharModal}
			on:keydown={(e) => e.key === 'Escape' && fecharModal()}
			role="presentation"
		></div>

		<div class="relative bg-slate-900 border border-slate-700 rounded-2xl max-w-2xl w-full p-6 shadow-2xl space-y-5 animate-in fade-in zoom-in-95 duration-150 z-10">
			<!-- Header do Modal -->
			<div class="flex items-start justify-between gap-4 border-b border-slate-800 pb-4">
				<div class="space-y-1">
					<div class="flex items-center gap-2">
						<span class={`px-2.5 py-0.5 text-xs font-bold rounded-lg border ${modalEstilo.bg} ${modalEstilo.text} ${modalEstilo.border}`}>
							Severidade {alertaSelecionado.severidade}
						</span>
						<span class="px-2 py-0.5 text-xs font-semibold rounded bg-slate-800 text-slate-300 border border-slate-700">
							{alertaSelecionado.tipo}
						</span>
					</div>
					<h2 id="modal-titulo" class="text-lg font-bold text-white tracking-tight mt-1">
						{alertaSelecionado.titulo}
					</h2>
				</div>

				<button
					type="button"
					on:click={fecharModal}
					class="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-800 transition-colors"
					title="Fechar modal"
				>
					<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
					</svg>
				</button>
			</div>

			<!-- Descrição e Justificativa -->
			<div class="space-y-3">
				<div>
					<h4 class="text-xs font-bold uppercase text-slate-400 tracking-wider">Descrição da Anomalia</h4>
					<p class="text-sm text-slate-200 mt-1 leading-relaxed bg-slate-800/50 p-3.5 rounded-xl border border-slate-800">
						{alertaSelecionado.descricao}
					</p>
				</div>

				<!-- Grid de Informações -->
				<div class="grid grid-cols-1 sm:grid-cols-2 gap-3 text-xs">
					<div class="p-3 bg-slate-800/40 rounded-lg border border-slate-800">
						<span class="text-slate-400 block">Alvo Auditado:</span>
						<span class="font-semibold text-slate-100 text-sm">{alertaSelecionado.alvo_nome}</span>
						{#if alertaSelecionado.alvo_documento}
							<span class="font-mono text-slate-400 block mt-0.5">{alertaSelecionado.alvo_documento}</span>
						{/if}
					</div>

					<div class="p-3 bg-slate-800/40 rounded-lg border border-slate-800">
						<span class="text-slate-400 block">Valor Envolvido:</span>
						<span class="font-bold text-emerald-400 text-sm font-mono">
							{formatarMoeda(alertaSelecionado.valor_envolvido)}
						</span>
						{#if alertaSelecionado.ano}
							<span class="text-slate-400 block mt-0.5">Exercício / Ano: {alertaSelecionado.ano}</span>
						{/if}
					</div>

					<div class="p-3 bg-slate-800/40 rounded-lg border border-slate-800">
						<span class="text-slate-400 block">Fonte Primária de Dados:</span>
						<span class="font-semibold text-slate-200">{alertaSelecionado.fonte_dado}</span>
						<span class="text-slate-500 block mt-0.5">Base pública oficial do governo</span>
					</div>

					<div class="p-3 bg-slate-800/40 rounded-lg border border-slate-800">
						<span class="text-slate-400 block">Localização / UF:</span>
						<span class="font-semibold text-slate-200">
							{alertaSelecionado.municipio || 'Âmbito Nacional'} {alertaSelecionado.uf ? `(${alertaSelecionado.uf})` : ''}
						</span>
					</div>
				</div>

				<!-- Detalhes Técnicos JSON (se disponíveis) -->
				{#if alertaSelecionado.detalhes}
					<div>
						<h4 class="text-xs font-bold uppercase text-slate-400 tracking-wider mb-1">Metadados Analíticos</h4>
						<pre class="p-3 bg-slate-950 rounded-xl border border-slate-800 text-[11px] font-mono text-slate-300 overflow-x-auto max-h-40">{JSON.stringify(alertaSelecionado.detalhes, null, 2)}</pre>
					</div>
				{/if}
			</div>

			<!-- Footer do Modal -->
			<div class="pt-3 border-t border-slate-800 flex justify-end">
				<button
					type="button"
					on:click={fecharModal}
					class="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-semibold rounded-lg transition-colors"
				>
					Fechar
				</button>
			</div>
		</div>
	</div>
{/if}
