<script lang="ts">
	import { onMount } from 'svelte';

	interface TotalRegistros {
		politicos: number;
		candidaturas: number;
		receitas_campanha: number;
		despesas_campanha: number;
		despesas_parlamentares: number;
		contratos_publicos: number;
		alertas_auditoria: number;
	}

	interface ConfigStatus {
		tamanho_banco_bytes: number;
		tamanho_banco_formatado: string;
		caminho_banco: string;
		total_registros: TotalRegistros;
		ultimo_evento_sincronizacao: string | null;
		versao_sistema: string;
	}

	interface JobInfo {
		job_id: string;
		fonte: string;
		ano?: number;
		status: 'PENDENTE' | 'PROCESSANDO' | 'CONCLUIDO' | 'ERRO';
		progresso: number;
		mensagem: string;
		logs: string[];
		criado_em: string;
		concluido_em?: string;
	}

	interface UploadResult {
		status: string;
		arquivo: string;
		tipo_detectado: string;
		registros_inseridos: number;
		mensagem: string;
	}

	let status: ConfigStatus | null = null;
	let loadingStatus = true;
	let erroStatus: string | null = null;

	// Ingestão
	let anoTse = 2024;
	let anoCeap = 2024;
	let anoPncp = 2024;
	let disparandoFonte: string | null = null;
	let verificandoTse = false;
	let msgTseVerificacao: string | null = null;

	// Monitor de Job
	let activeJob: JobInfo | null = null;
	let pollingInterval: any = null;

	// Upload Manual
	let tipoDocumentoUpload = 'AUTO';
	let arquivoSelecionado: File | null = null;
	let isDragging = false;
	let enviandoUpload = false;
	let resultadoUpload: UploadResult | null = null;
	let erroUpload: string | null = null;

	// Exportação
	let tabelaExportar = 'politicos';
	let formatoExportar = 'csv';
	let exportando = false;

	const tabelasDisponiveis = [
		{ id: 'politicos', nome: 'Políticos (TSE)' },
		{ id: 'candidaturas', nome: 'Candidaturas' },
		{ id: 'receitas_campanha', nome: 'Receitas de Campanha' },
		{ id: 'despesas_campanha', nome: 'Despesas de Campanha' },
		{ id: 'despesas_parlamentares', nome: 'Despesas CEAP (Câmara)' },
		{ id: 'contratos_publicos', nome: 'Contratos Públicos (PNCP)' },
		{ id: 'alertas_auditoria', nome: 'Alertas de Auditoria' },
		{ id: 'empresas_qsa', nome: 'Quadro Societário (QSA)' },
		{ id: 'bens_candidato', nome: 'Bens Declarados' },
		{ id: 'nos_rede', nome: 'Nós do Grafo Relacional' },
		{ id: 'conexoes_rede', nome: 'Arestas / Vínculos de Rede' },
		{ id: 'historico_sincronizacao', nome: 'Histórico de Sincronizações' }
	];

	async function carregarStatus() {
		loadingStatus = true;
		erroStatus = null;
		try {
			const res = await fetch('/api/v1/config/status');
			if (!res.ok) {
				throw new Error(`Erro ${res.status}: ${res.statusText}`);
			}
			status = await res.json();
		} catch (e: any) {
			erroStatus = e.message || 'Falha ao conectar à API de status';
		} finally {
			loadingStatus = false;
		}
	}

	async function verificarDisponibilidadeTse() {
		verificandoTse = true;
		msgTseVerificacao = null;
		try {
			const res = await fetch(`/api/v1/config/tse/verificar/${anoTse}`);
			const data = await res.json();
			msgTseVerificacao = data.mensagem || `Verificação concluída para ${anoTse}`;
		} catch (e: any) {
			msgTseVerificacao = `Erro ao verificar repositório: ${e.message}`;
		} finally {
			verificandoTse = false;
		}
	}

	async function dispararIngestao(fonte: string, ano?: number) {
		disparandoFonte = fonte;
		try {
			const res = await fetch('/api/v1/config/ingestao/executar', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ fonte, ano })
			});

			if (!res.ok) {
				const err = await res.json();
				throw new Error(err.erro || 'Falha ao despachar job de ingestão');
			}

			const data = await res.json();
			iniciarPollingJob(data.job_id);
		} catch (e: any) {
			alert(`Erro ao iniciar ingestão: ${e.message}`);
		} finally {
			disparandoFonte = null;
		}
	}

	function iniciarPollingJob(jobId: string) {
		if (pollingInterval) clearInterval(pollingInterval);

		const verificar = async () => {
			try {
				const res = await fetch(`/api/v1/config/ingestao/status/${jobId}`);
				if (res.ok) {
					activeJob = await res.json();
					if (activeJob?.status === 'CONCLUIDO' || activeJob?.status === 'ERRO') {
						clearInterval(pollingInterval);
						pollingInterval = null;
						carregarStatus();
					}
				}
			} catch (e) {
				console.error('Erro no polling do job:', e);
			}
		};

		verificar();
		pollingInterval = setInterval(verificar, 1000);
	}

	function handleFileDrop(e: DragEvent) {
		e.preventDefault();
		isDragging = false;
		if (e.dataTransfer && e.dataTransfer.files.length > 0) {
			arquivoSelecionado = e.dataTransfer.files[0];
			resultadoUpload = null;
			erroUpload = null;
		}
	}

	function handleFileSelect(e: Event) {
		const target = e.target as HTMLInputElement;
		if (target.files && target.files.length > 0) {
			arquivoSelecionado = target.files[0];
			resultadoUpload = null;
			erroUpload = null;
		}
	}

	async function enviarUpload() {
		if (!arquivoSelecionado) return;

		enviandoUpload = true;
		erroUpload = null;
		resultadoUpload = null;

		try {
			const formData = new FormData();
			formData.append('arquivo', arquivoSelecionado);
			formData.append('tipo', tipoDocumentoUpload);

			const res = await fetch('/api/v1/config/ingestao/upload', {
				method: 'POST',
				body: formData
			});

			if (!res.ok) {
				const err = await res.json();
				throw new Error(err.erro || 'Falha no processamento do upload');
			}

			resultadoUpload = await res.json();
			arquivoSelecionado = null;
			carregarStatus();
		} catch (e: any) {
			erroUpload = e.message || 'Erro ao realizar upload do arquivo';
		} finally {
			enviandoUpload = false;
		}
	}

	function baixarBanco() {
		window.location.href = '/api/v1/config/exportar/banco';
	}

	function baixarTabela() {
		window.location.href = `/api/v1/config/exportar/tabela/${tabelaExportar}?formato=${formatoExportar}`;
	}

	onMount(() => {
		carregarStatus();
		return () => {
			if (pollingInterval) clearInterval(pollingInterval);
		};
	});
</script>

<svelte:head>
	<title>Configurações & Gestão de Dados - Radar Cívico</title>
</svelte:head>

<div class="space-y-10">
	<!-- Cabeçalho -->
	<div class="flex flex-col md:flex-row md:items-center md:justify-between gap-4 border-b border-slate-800 pb-6">
		<div>
			<div class="flex items-center gap-3">
				<div class="p-2 rounded-lg bg-indigo-500/10 border border-indigo-500/30 text-indigo-400">
					<svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
					</svg>
				</div>
				<div>
					<h1 class="text-2xl font-bold tracking-tight text-white">Configurações e Gestão de Dados</h1>
					<p class="text-sm text-slate-400">Mini Data Lake SQLite, Ingestão Atemporal e Exportação de Dumps</p>
				</div>
			</div>
		</div>

		<div class="flex items-center gap-3">
			{#if status?.ultimo_evento_sincronizacao}
				<div class="hidden lg:flex items-center gap-2 px-3 py-1.5 bg-slate-800/80 border border-slate-700/60 rounded-lg text-xs text-slate-300">
					<span class="w-2 h-2 rounded-full bg-emerald-400"></span>
					<span>Último sync: <strong class="text-white">{status.ultimo_evento_sincronizacao}</strong></span>
				</div>
			{/if}

			<button
				on:click={carregarStatus}
				disabled={loadingStatus}
				class="inline-flex items-center gap-2 px-3.5 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 text-sm font-medium rounded-lg border border-slate-700 transition-colors disabled:opacity-50"
			>
				<svg class="w-4 h-4 {loadingStatus ? 'animate-spin text-emerald-400' : 'text-slate-400'}" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
				</svg>
				<span>{loadingStatus ? 'Atualizando...' : 'Atualizar Métricas'}</span>
			</button>
		</div>
	</div>

	{#if erroStatus}
		<div class="p-4 bg-rose-500/10 border border-rose-500/30 rounded-xl text-rose-300 text-sm flex items-center justify-between">
			<div class="flex items-center gap-2">
				<svg class="w-5 h-5 text-rose-400 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
				</svg>
				<span>{erroStatus}</span>
			</div>
			<button on:click={carregarStatus} class="underline text-xs hover:text-white">Tentar novamente</button>
		</div>
	{/if}

	<!-- Seção 1: Cards de Diagnóstico do Mini Data Lake -->
	<div>
		<h2 class="text-base font-semibold text-slate-200 mb-4 flex items-center gap-2">
			<span class="w-2 h-2 rounded-full bg-indigo-400"></span>
			Diagnóstico do Mini Data Lake SQLite
		</h2>

		<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
			<!-- Card 1: Tamanho SQLite -->
			<div class="p-5 bg-slate-800/70 border border-slate-700/60 rounded-xl flex items-center gap-4">
				<div class="w-12 h-12 rounded-lg bg-indigo-500/10 border border-indigo-500/30 flex items-center justify-center text-indigo-400">
					<svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4m0 5c0 2.21-3.582 4-8 4s-8-1.79-8-4" />
					</svg>
				</div>
				<div>
					<p class="text-xs font-medium text-slate-400 uppercase tracking-wider">Tamanho do Banco</p>
					<p class="text-2xl font-bold text-white mt-0.5">
						{#if loadingStatus && !status}
							<span class="text-slate-500 text-lg">Carregando...</span>
						{:else}
							{status?.tamanho_banco_formatado || '0 B'}
						{/if}
					</p>
					<p class="text-xs text-slate-400 truncate max-w-[180px]" title={status?.caminho_banco || ''}>
						{status?.caminho_banco || 'sqlite local'}
					</p>
				</div>
			</div>

			<!-- Card 2: Políticos Catalogados -->
			<div class="p-5 bg-slate-800/70 border border-slate-700/60 rounded-xl flex items-center gap-4">
				<div class="w-12 h-12 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400">
					<svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
					</svg>
				</div>
				<div>
					<p class="text-xs font-medium text-slate-400 uppercase tracking-wider">Políticos Mapeados</p>
					<p class="text-2xl font-bold text-white mt-0.5">
						{#if loadingStatus && !status}
							<span class="text-slate-500 text-lg">...</span>
						{:else}
							{(status?.total_registros.politicos ?? 0).toLocaleString('pt-BR')}
						{/if}
					</p>
					<p class="text-xs text-slate-400">
						{(status?.total_registros.candidaturas ?? 0).toLocaleString('pt-BR')} candidaturas
					</p>
				</div>
			</div>

			<!-- Card 3: Receitas & Despesas -->
			<div class="p-5 bg-slate-800/70 border border-slate-700/60 rounded-xl flex items-center gap-4">
				<div class="w-12 h-12 rounded-lg bg-amber-500/10 border border-amber-500/30 flex items-center justify-center text-amber-400">
					<svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
					</svg>
				</div>
				<div>
					<p class="text-xs font-medium text-slate-400 uppercase tracking-wider">Receitas Eleitorais</p>
					<p class="text-2xl font-bold text-white mt-0.5">
						{#if loadingStatus && !status}
							<span class="text-slate-500 text-lg">...</span>
						{:else}
							{(status?.total_registros.receitas_campanha ?? 0).toLocaleString('pt-BR')}
						{/if}
					</p>
					<p class="text-xs text-slate-400">
						{(status?.total_registros.despesas_campanha ?? 0).toLocaleString('pt-BR')} despesas de campanha
					</p>
				</div>
			</div>

			<!-- Card 4: Contratos & Anomalias -->
			<div class="p-5 bg-slate-800/70 border border-slate-700/60 rounded-xl flex items-center gap-4">
				<div class="w-12 h-12 rounded-lg bg-rose-500/10 border border-rose-500/30 flex items-center justify-center text-rose-400">
					<svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
					</svg>
				</div>
				<div>
					<p class="text-xs font-medium text-slate-400 uppercase tracking-wider">Alertas de Auditoria</p>
					<p class="text-2xl font-bold text-white mt-0.5">
						{#if loadingStatus && !status}
							<span class="text-slate-500 text-lg">...</span>
						{:else}
							{(status?.total_registros.alertas_auditoria ?? 0).toLocaleString('pt-BR')}
						{/if}
					</p>
					<p class="text-xs text-slate-400">
						{(status?.total_registros.contratos_publicos ?? 0).toLocaleString('pt-BR')} contratos PNCP
					</p>
				</div>
			</div>
		</div>
	</div>

	<!-- Seção 2: Painel de Controle de Fontes Públicas -->
	<div>
		<h2 class="text-base font-semibold text-slate-200 mb-4 flex items-center gap-2">
			<span class="w-2 h-2 rounded-full bg-emerald-400"></span>
			Disparo de Ingestão Online (Background Tasks)
		</h2>

		<div class="grid grid-cols-1 md:grid-cols-2 gap-6">
			<!-- Fonte 1: TSE -->
			<div class="p-6 bg-slate-800/50 border border-slate-700/60 rounded-xl space-y-4">
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-3">
						<div class="w-9 h-9 rounded-lg bg-blue-500/10 border border-blue-500/30 flex items-center justify-center text-blue-400 font-bold text-xs">
							TSE
						</div>
						<div>
							<h3 class="font-medium text-white">Tribunal Superior Eleitoral</h3>
							<p class="text-xs text-slate-400">Candidaturas, bens e prestação de contas de campanha</p>
						</div>
					</div>
				</div>

				<div class="flex flex-wrap items-center gap-3 pt-2">
					<div class="flex items-center gap-2">
						<label for="ano-tse" class="text-xs text-slate-400">Ano Eleitoral:</label>
						<select
							id="ano-tse"
							bind:value={anoTse}
							class="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-2.5 py-1.5 focus:ring-emerald-500 focus:border-emerald-500"
						>
							<option value={2026}>2026 (Projeção)</option>
							<option value={2024}>2024 (Municipal)</option>
							<option value={2022}>2022 (Geral)</option>
							<option value={2020}>2020 (Municipal)</option>
							<option value={2018}>2018 (Geral)</option>
						</select>
					</div>

					<button
						on:click={verificarDisponibilidadeTse}
						disabled={verificandoTse}
						class="px-3 py-1.5 bg-slate-700/80 hover:bg-slate-700 text-slate-300 text-xs font-medium rounded-lg border border-slate-600 transition-colors disabled:opacity-50"
					>
						{verificandoTse ? 'Verificando...' : 'Checar Disponibilidade'}
					</button>

					<button
						on:click={() => dispararIngestao('TSE', anoTse)}
						disabled={disparandoFonte === 'TSE'}
						class="px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50"
					>
						{disparandoFonte === 'TSE' ? 'Despachando...' : 'Sincronizar Dados'}
					</button>
				</div>

				{#if msgTseVerificacao}
					<p class="text-xs text-slate-300 bg-slate-900/60 p-2.5 rounded-lg border border-slate-700/50">
						ℹ️ {msgTseVerificacao}
					</p>
				{/if}
			</div>

			<!-- Fonte 2: CEAP (Câmara dos Deputados) -->
			<div class="p-6 bg-slate-800/50 border border-slate-700/60 rounded-xl space-y-4">
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-3">
						<div class="w-9 h-9 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 font-bold text-xs">
							CEAP
						</div>
						<div>
							<h3 class="font-medium text-white">Câmara dos Deputados (CEAP)</h3>
							<p class="text-xs text-slate-400">Reembolsos parlamentares, combustíveis e notas fiscais</p>
						</div>
					</div>
				</div>

				<div class="flex flex-wrap items-center gap-3 pt-2">
					<div class="flex items-center gap-2">
						<label for="ano-ceap" class="text-xs text-slate-400">Ano Fiscal:</label>
						<select
							id="ano-ceap"
							bind:value={anoCeap}
							class="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-2.5 py-1.5 focus:ring-emerald-500 focus:border-emerald-500"
						>
							<option value={2024}>2024</option>
							<option value={2023}>2023</option>
							<option value={2022}>2022</option>
							<option value={2021}>2021</option>
							<option value={2020}>2020</option>
						</select>
					</div>

					<button
						on:click={() => dispararIngestao('CEAP', anoCeap)}
						disabled={disparandoFonte === 'CEAP'}
						class="px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50"
					>
						{disparandoFonte === 'CEAP' ? 'Despachando...' : 'Puxar Notas Fiscais'}
					</button>
				</div>
			</div>

			<!-- Fonte 3: Receita Federal (QSA) -->
			<div class="p-6 bg-slate-800/50 border border-slate-700/60 rounded-xl space-y-4">
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-3">
						<div class="w-9 h-9 rounded-lg bg-amber-500/10 border border-amber-500/30 flex items-center justify-center text-amber-400 font-bold text-xs">
							QSA
						</div>
						<div>
							<h3 class="font-medium text-white">Receita Federal (QSA)</h3>
							<p class="text-xs text-slate-400">Quadro de Sócios, Administradores e situação cadastral</p>
						</div>
					</div>
				</div>

				<div class="flex items-center gap-3 pt-2">
					<button
						on:click={() => dispararIngestao('RECEITA_QSA')}
						disabled={disparandoFonte === 'RECEITA_QSA'}
						class="px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50"
					>
						{disparandoFonte === 'RECEITA_QSA' ? 'Despachando...' : 'Ingerir Sócios e CNPJs'}
					</button>
				</div>
			</div>

			<!-- Fonte 4: PNCP (Contratos Públicos) -->
			<div class="p-6 bg-slate-800/50 border border-slate-700/60 rounded-xl space-y-4">
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-3">
						<div class="w-9 h-9 rounded-lg bg-purple-500/10 border border-purple-500/30 flex items-center justify-center text-purple-400 font-bold text-xs">
							PNCP
						</div>
						<div>
							<h3 class="font-medium text-white">Portal Nacional de Contratações (PNCP)</h3>
							<p class="text-xs text-slate-400">Licitações municipais/estaduais, atas e contratos</p>
						</div>
					</div>
				</div>

				<div class="flex flex-wrap items-center gap-3 pt-2">
					<div class="flex items-center gap-2">
						<label for="ano-pncp" class="text-xs text-slate-400">Ano Contrato:</label>
						<select
							id="ano-pncp"
							bind:value={anoPncp}
							class="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-2.5 py-1.5 focus:ring-emerald-500 focus:border-emerald-500"
						>
							<option value={2024}>2024</option>
							<option value={2023}>2023</option>
							<option value={2022}>2022</option>
						</select>
					</div>

					<button
						on:click={() => dispararIngestao('PNCP', anoPncp)}
						disabled={disparandoFonte === 'PNCP'}
						class="px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50"
					>
						{disparandoFonte === 'PNCP' ? 'Despachando...' : 'Atualizar Contratos'}
					</button>
				</div>
			</div>
		</div>
	</div>

	<!-- Seção 3: Monitor de Job Ativo em Tempo Real -->
	{#if activeJob}
		<div class="p-6 bg-slate-800/70 border border-slate-700 rounded-xl space-y-4 animate-fade-in">
			<div class="flex items-center justify-between">
				<div class="flex items-center gap-3">
					<span class="text-sm font-semibold text-white">Monitor de Execução:</span>
					<span class="px-2 py-0.5 text-xs font-mono bg-slate-900 text-slate-300 rounded border border-slate-700">
						{activeJob.job_id}
					</span>
					<span class="text-xs font-semibold px-2 py-0.5 rounded uppercase
						{activeJob.status === 'CONCLUIDO' ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30' :
						 activeJob.status === 'ERRO' ? 'bg-rose-500/20 text-rose-400 border border-rose-500/30' :
						 'bg-amber-500/20 text-amber-400 border border-amber-500/30 animate-pulse'}">
						{activeJob.status}
					</span>
				</div>
				<span class="text-xs text-slate-400 font-mono">{activeJob.progresso}%</span>
			</div>

			<!-- Barra de Progresso -->
			<div class="w-full bg-slate-900 rounded-full h-2.5 overflow-hidden border border-slate-700/60">
				<div
					class="h-2.5 rounded-full transition-all duration-300
						{activeJob.status === 'CONCLUIDO' ? 'bg-emerald-500' :
						 activeJob.status === 'ERRO' ? 'bg-rose-500' : 'bg-emerald-500'}"
					style="width: {activeJob.progresso}%"
				></div>
			</div>

			<!-- Terminal de Logs -->
			<div class="bg-slate-950 p-4 rounded-lg border border-slate-800 font-mono text-xs text-slate-300 space-y-1 max-h-48 overflow-y-auto">
				{#each activeJob.logs as log}
					<p class="leading-relaxed">{log}</p>
				{/each}
			</div>
		</div>
	{/if}

	<!-- Seção 4: Área de Importação Manual (Dropzone) -->
	<div>
		<h2 class="text-base font-semibold text-slate-200 mb-4 flex items-center gap-2">
			<span class="w-2 h-2 rounded-full bg-cyan-400"></span>
			Carga Manual Offline (Streams Assíncronos CSV / ZIP)
		</h2>

		<div class="bg-slate-800/40 border border-slate-700/60 rounded-xl p-6 space-y-6">
			<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
				<div>
					<p class="text-sm font-medium text-white">Carregue arquivos oficiais (.zip ou .csv)</p>
					<p class="text-xs text-slate-400">Processamento em streaming contínuo sem carregar o arquivo na memória RAM</p>
				</div>

				<div class="flex items-center gap-2">
					<label for="tipo-doc-upload" class="text-xs text-slate-400">Destino:</label>
					<select
						id="tipo-doc-upload"
						bind:value={tipoDocumentoUpload}
						class="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-1.5 focus:ring-emerald-500 focus:border-emerald-500"
					>
						<option value="AUTO">Detecção Automática de Cabeçalho</option>
						<option value="TSE_CANDIDATOS">TSE - Candidatos (consulta_cand)</option>
						<option value="TSE_RECEITAS">TSE - Receitas de Campanha</option>
						<option value="TSE_DESPESAS">TSE - Despesas de Campanha</option>
						<option value="CEAP">Câmara - Notas Fiscais CEAP</option>
						<option value="PNCP">PNCP - Contratos e Licitações</option>
					</select>
				</div>
			</div>

			<!-- Dropzone Area -->
			<div
				role="region"
				aria-label="Área de upload de arquivos"
				on:dragover|preventDefault={() => (isDragging = true)}
				on:dragleave|preventDefault={() => (isDragging = false)}
				on:drop={handleFileDrop}
				class="border-2 border-dashed rounded-xl p-8 text-center transition-colors
					{isDragging ? 'border-emerald-500 bg-emerald-500/5' : 'border-slate-700 hover:border-slate-600 bg-slate-900/40'}"
			>
				<input
					type="file"
					id="file-input"
					accept=".csv,.zip"
					on:change={handleFileSelect}
					class="hidden"
				/>

				<div class="flex flex-col items-center justify-center gap-2">
					<div class="w-12 h-12 rounded-full bg-slate-800 flex items-center justify-center text-slate-400">
						<svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
						</svg>
					</div>

					{#if arquivoSelecionado}
						<div class="mt-2 text-center">
							<p class="text-sm font-semibold text-emerald-400">{arquivoSelecionado.name}</p>
							<p class="text-xs text-slate-400 mt-0.5">{(arquivoSelecionado.size / 1024 / 1024).toFixed(2)} MB</p>
						</div>
					{:else}
						<p class="text-sm font-medium text-slate-300 mt-2">
							Arraste e solte seu arquivo <span class="text-emerald-400 font-mono">.csv</span> ou <span class="text-emerald-400 font-mono">.zip</span> aqui
						</p>
						<p class="text-xs text-slate-500">ou</p>
					{/if}

					<div class="mt-2 flex items-center gap-3">
						<label
							for="file-input"
							class="cursor-pointer px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium rounded-lg border border-slate-700 transition-colors"
						>
							{arquivoSelecionado ? 'Alterar Arquivo' : 'Selecionar do Computador'}
						</label>

						{#if arquivoSelecionado}
							<button
								on:click={enviarUpload}
								disabled={enviandoUpload}
								class="px-5 py-2 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50 flex items-center gap-2"
							>
								{#if enviandoUpload}
									<svg class="w-4 h-4 animate-spin text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
										<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
									</svg>
									<span>Processando em Streaming...</span>
								{:else}
									<span>Iniciar Carga no SQLite</span>
								{/if}
							</button>
						{/if}
					</div>
				</div>
			</div>

			{#if resultadoUpload}
				<div class="p-4 bg-emerald-500/10 border border-emerald-500/30 rounded-xl text-emerald-300 text-xs space-y-1">
					<p class="font-bold text-sm text-emerald-400">✅ Carga Finalizada com Sucesso!</p>
					<p>{resultadoUpload.mensagem}</p>
					<p class="text-slate-400">Tipo Detectado: <span class="font-mono text-emerald-300">{resultadoUpload.tipo_detectado}</span> | Registros Inseridos: <strong class="text-white">{resultadoUpload.registros_inseridos}</strong></p>
				</div>
			{/if}

			{#if erroUpload}
				<div class="p-4 bg-rose-500/10 border border-rose-500/30 rounded-xl text-rose-300 text-xs">
					<p class="font-bold text-rose-400">❌ Falha na Carga:</p>
					<p>{erroUpload}</p>
				</div>
			{/if}
		</div>
	</div>

	<!-- Seção 5: Área de Backup e Exportação -->
	<div>
		<h2 class="text-base font-semibold text-slate-200 mb-4 flex items-center gap-2">
			<span class="w-2 h-2 rounded-full bg-amber-400"></span>
			Backup e Exportação de Dumps
		</h2>

		<div class="grid grid-cols-1 md:grid-cols-2 gap-6">
			<!-- Backup Completo do SQLite -->
			<div class="p-6 bg-slate-800/50 border border-slate-700/60 rounded-xl space-y-4">
				<div class="flex items-center gap-3">
					<div class="w-10 h-10 rounded-lg bg-amber-500/10 border border-amber-500/30 flex items-center justify-center text-amber-400">
						<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
						</svg>
					</div>
					<div>
						<h3 class="font-medium text-white">Snapshot do Banco SQLite Completo</h3>
						<p class="text-xs text-slate-400">Cópia íntegra de produção (.sqlite) gerada sem travar transações ativas</p>
					</div>
				</div>

				<div class="pt-2">
					<button
						on:click={baixarBanco}
						class="w-full sm:w-auto px-5 py-2.5 bg-amber-600 hover:bg-amber-500 text-slate-950 font-semibold text-xs rounded-lg transition-colors flex items-center justify-center gap-2"
					>
						<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
						</svg>
						<span>Exportar Base Completa (.sqlite)</span>
					</button>
				</div>
			</div>

			<!-- Exportação de Tabela em CSV ou JSON -->
			<div class="p-6 bg-slate-800/50 border border-slate-700/60 rounded-xl space-y-4">
				<div class="flex items-center gap-3">
					<div class="w-10 h-10 rounded-lg bg-indigo-500/10 border border-indigo-500/30 flex items-center justify-center text-indigo-400">
						<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
						</svg>
					</div>
					<div>
						<h3 class="font-medium text-white">Exportação de Tabela Específica</h3>
						<p class="text-xs text-slate-400">Download em lote de qualquer tabela para análise em Python/Pandas</p>
					</div>
				</div>

				<div class="flex flex-wrap items-center gap-3 pt-2">
					<select
						bind:value={tabelaExportar}
						class="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:ring-emerald-500 focus:border-emerald-500"
					>
						{#each tabelasDisponiveis as t}
							<option value={t.id}>{t.nome}</option>
						{/each}
					</select>

					<div class="flex items-center bg-slate-900 rounded-lg border border-slate-700 p-0.5 text-xs">
						<button
							type="button"
							on:click={() => (formatoExportar = 'csv')}
							class="px-2.5 py-1.5 rounded-md font-medium transition-colors {formatoExportar === 'csv' ? 'bg-indigo-600 text-white' : 'text-slate-400 hover:text-white'}"
						>
							CSV
						</button>
						<button
							type="button"
							on:click={() => (formatoExportar = 'json')}
							class="px-2.5 py-1.5 rounded-md font-medium transition-colors {formatoExportar === 'json' ? 'bg-indigo-600 text-white' : 'text-slate-400 hover:text-white'}"
						>
							JSON
						</button>
					</div>

					<button
						on:click={baixarTabela}
						class="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white font-semibold text-xs rounded-lg transition-colors flex items-center gap-2"
					>
						<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
						</svg>
						<span>Baixar {formatoExportar.toUpperCase()}</span>
					</button>
				</div>
			</div>
		</div>
	</div>
</div>
