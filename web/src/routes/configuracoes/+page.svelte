<script lang="ts">
	import { onMount } from 'svelte';
	import { versionStore, sincronizarVersaoServidor } from '$lib/version';

	interface TotalRegistros {
		politicos: number;
		candidaturas: number;
		receitas_campanha: number;
		despesas_campanha: number;
		despesas_parlamentares: number;
		contratos_publicos: number;
		alertas_auditoria: number;
		empresas_qsa?: number;
		registros_profissionais?: number;
	}

	interface ConfigStatus {
		tamanho_banco_bytes: number;
		tamanho_banco_formatado: string;
		caminho_banco: string;
		total_registros: TotalRegistros;
		ultimo_evento_sincronizacao: string | null;
		versao_sistema: string;
		git_commit?: string;
		commit_count?: number;
	}

	interface IdentidadeVisual {
		tem_icone_customizado: boolean;
		tem_favicon_customizado: boolean;
		icone_url: string;
		favicon_url: string;
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

	// Identidade Visual & Ícones
	let identidade: IdentidadeVisual | null = null;
	let arquivoIcone: File | null = null;
	let previewIconeUrl: string | null = null;
	let alvoIcone: 'ambos' | 'icone' | 'favicon' = 'ambos';
	let salvandoIcone = false;
	let resetandoIcone = false;
	let msgIconeSucesso: string | null = null;
	let erroIcone: string | null = null;
	let iconTimestamp = Date.now();
	let mostrarGuiaCabecalhos = false;

	// Ingestão
	let anoTse = 2024;
	let sincronizandoTse = false;
	let descobrirViaCkanTse = true;
	let anoCeap = 2024;
	let modoCeap: 'BULK' | 'API' = 'BULK';
	let sincronizandoCamara = false;
	let idsDeputadosCeap = '';
	let maxPaginasCeap = 5;
	let anoPncp = 2024;
	let disparandoFonte: string | null = null;
	let verificandoTse = false;
	let msgTseVerificacao: string | null = null;

	// Monitor de Job & Ingestão
	let activeJob: JobInfo | null = null;
	let pollingInterval: any = null;

	interface ImportProgress {
		is_running: boolean;
		current_file: string;
		files_processed: number;
		total_files: number;
		records_processed: number;
		percentage: number;
		started_at?: string;
		elapsed_seconds: number;
		last_error?: string;
	}
	let importProgress: ImportProgress | null = null;

	// Ingestão Unificada (SourceImporter)
	interface ImporterSummary {
		id: string;
		name: string;
		description: string;
		stage: 'IDLE' | 'CONECTANDO' | 'BAIXANDO' | 'DESCOMPACTANDO' | 'PROCESSANDO' | 'FINALIZANDO' | 'CONCLUIDO' | 'CANCELADO' | 'ERRO';
		is_running: boolean;
		current_file: string;
		files_processed: number;
		total_files: number;
		records_processed: number;
		percentage: number;
		elapsed_seconds: number;
		message: string;
		last_error?: string | null;
	}
	let importers: ImporterSummary[] = [];
	let acaoImporterId: string | null = null;
	let importersInterval: any = null;

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
		{ id: 'registros_profissionais', nome: 'Registros Profissionais (OAB)' },
		{ id: 'bens_candidato', nome: 'Bens Declarados' },
		{ id: 'nos_rede', nome: 'Nós do Grafo Relacional' },
		{ id: 'conexoes_rede', nome: 'Arestas / Vínculos de Rede' },
		{ id: 'historico_sincronizacao', nome: 'Histórico de Sincronizações' },
		{ id: 'configuracoes_sistema', nome: 'Configurações de Identidade' }
	];

	async function carregarIdentidade() {
		try {
			const res = await fetch('/api/v1/config/identidade');
			if (res.ok) {
				identidade = await res.json();
			}
		} catch (e) {
			console.error('Erro ao carregar identidade visual:', e);
		}
	}

	function handleIconSelect(e: Event) {
		const target = e.target as HTMLInputElement;
		if (target.files && target.files.length > 0) {
			arquivoIcone = target.files[0];
			erroIcone = null;
			msgIconeSucesso = null;
			if (previewIconeUrl) URL.revokeObjectURL(previewIconeUrl);
			previewIconeUrl = URL.createObjectURL(arquivoIcone);
		}
	}

	async function salvarIcone() {
		if (!arquivoIcone) return;
		salvandoIcone = true;
		erroIcone = null;
		msgIconeSucesso = null;

		try {
			const formData = new FormData();
			formData.append('arquivo', arquivoIcone);
			formData.append('alvo', alvoIcone);

			const res = await fetch('/api/v1/config/icone', {
				method: 'POST',
				body: formData
			});

			if (!res.ok) {
				const err = await res.json();
				throw new Error(err.erro || 'Falha ao salvar ícone');
			}

			msgIconeSucesso = 'Identidade visual atualizada com sucesso!';
			arquivoIcone = null;
			if (previewIconeUrl) {
				URL.revokeObjectURL(previewIconeUrl);
				previewIconeUrl = null;
			}
			iconTimestamp = Date.now();
			await carregarIdentidade();
			window.dispatchEvent(new CustomEvent('radar-icon-updated'));
		} catch (e: any) {
			erroIcone = e.message || 'Erro ao enviar imagem';
		} finally {
			salvandoIcone = false;
		}
	}

	async function restaurarIconePadrao() {
		resetandoIcone = true;
		erroIcone = null;
		msgIconeSucesso = null;

		try {
			const res = await fetch('/api/v1/config/icone?alvo=ambos', {
				method: 'DELETE'
			});

			if (!res.ok) {
				const err = await res.json();
				throw new Error(err.erro || 'Falha ao restaurar ícone');
			}

			msgIconeSucesso = 'Ícone restaurado para o padrão do Radar Cívico!';
			arquivoIcone = null;
			if (previewIconeUrl) {
				URL.revokeObjectURL(previewIconeUrl);
				previewIconeUrl = null;
			}
			iconTimestamp = Date.now();
			await carregarIdentidade();
			window.dispatchEvent(new CustomEvent('radar-icon-updated'));
		} catch (e: any) {
			erroIcone = e.message || 'Erro ao restaurar padrão';
		} finally {
			resetandoIcone = false;
		}
	}

	async function carregarStatus() {
		loadingStatus = true;
		erroStatus = null;
		try {
			const res = await fetch('/api/v1/config/status');
			if (!res.ok) {
				throw new Error(`Erro ${res.status}: ${res.statusText}`);
			}
			status = await res.json();
			if (status?.versao_sistema) {
				versionStore.update((v) => ({
					...v,
					version: status?.versao_sistema || v.version,
					commit: status?.git_commit || v.commit,
					count: status?.commit_count || v.count
				}));
			}
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

	async function dispararSincronizacaoTse() {
		sincronizandoTse = true;
		try {
			const res = await fetch('/api/v1/config/tse/sincronizar', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					ano: anoTse,
					descobrir_via_ckan: descobrirViaCkanTse
				})
			});

			if (!res.ok) {
				const err = await res.json();
				throw new Error(err.erro || 'Falha ao sincronizar dados do TSE');
			}

			const data = await res.json();
			iniciarPollingJob(data.job_id);
		} catch (e: any) {
			alert(`Erro na sincronização do TSE: ${e.message}`);
		} finally {
			sincronizandoTse = false;
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

	async function dispararSincronizacaoCamara() {
		sincronizandoCamara = true;
		try {
			const parsedIds = idsDeputadosCeap
				.split(',')
				.map((s) => parseInt(s.trim()))
				.filter((n) => !isNaN(n));

			const res = await fetch('/api/v1/config/camara/sincronizar', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					ano: anoCeap,
					modo: modoCeap,
					deputados_ids: parsedIds.length > 0 ? parsedIds : undefined,
					max_paginas: modoCeap === 'API' ? maxPaginasCeap : undefined
				})
			});

			if (!res.ok) {
				const err = await res.json();
				throw new Error(err.erro || 'Falha ao sincronizar dados da Câmara');
			}

			const data = await res.json();
			iniciarPollingJob(data.job_id);
		} catch (e: any) {
			alert(`Erro na sincronização da Câmara: ${e.message}`);
		} finally {
			sincronizandoCamara = false;
		}
	}

	function iniciarPollingJob(jobId: string) {
		if (pollingInterval) clearInterval(pollingInterval);

		const verificar = async () => {
			try {
				const [resJob, resProg] = await Promise.all([
					fetch(`/api/v1/config/ingestao/status/${jobId}`),
					fetch('/api/import/status')
				]);

				if (resJob.ok) {
					activeJob = await resJob.json();
				}
				if (resProg.ok) {
					importProgress = await resProg.json();
				}

				if (activeJob?.status === 'CONCLUIDO' || activeJob?.status === 'ERRO') {
					clearInterval(pollingInterval);
					pollingInterval = null;
					carregarStatus();
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

	async function carregarImporters() {
		try {
			const res = await fetch('/api/importers');
			if (res.ok) {
				importers = await res.json();
			}
		} catch (e) {
			console.error('Erro ao consultar /api/importers:', e);
		}
	}

	async function iniciarImporter(id: string) {
		acaoImporterId = id;
		try {
			const res = await fetch(`/api/importers/${id}/start`, { method: 'POST' });
			if (res.ok) {
				await carregarImporters();
			} else {
				const err = await res.json();
				alert(err.erro || 'Falha ao iniciar importação');
			}
		} catch (e: any) {
			alert(e.message || 'Erro de rede ao iniciar importação');
		} finally {
			acaoImporterId = null;
		}
	}

	async function cancelarImporter(id: string) {
		acaoImporterId = id;
		try {
			const res = await fetch(`/api/importers/${id}/cancel`, { method: 'POST' });
			if (res.ok) {
				await carregarImporters();
			} else {
				const err = await res.json();
				alert(err.erro || 'Falha ao cancelar importação');
			}
		} catch (e: any) {
			alert(e.message || 'Erro de rede ao cancelar importação');
		} finally {
			acaoImporterId = null;
		}
	}

	onMount(() => {
		carregarStatus();
		carregarIdentidade();
		carregarImporters();
		sincronizarVersaoServidor();
		importersInterval = setInterval(carregarImporters, 2000);
		return () => {
			if (pollingInterval) clearInterval(pollingInterval);
			if (importersInterval) clearInterval(importersInterval);
			if (previewIconeUrl) URL.revokeObjectURL(previewIconeUrl);
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
			<div class="hidden sm:flex items-center gap-2 px-3 py-1.5 bg-slate-800/80 border border-slate-700/60 rounded-lg text-xs text-slate-300">
				<span class="text-slate-400">Versão:</span>
				<span
					class="font-mono text-emerald-400 font-semibold cursor-help"
					title={`Commit Git: ${$versionStore.commit}`}
					aria-label={`Versão ${$versionStore.version}, commit ${$versionStore.commit}`}
				>
					{$versionStore.version}
				</span>
			</div>

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

		<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
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
						{(status?.total_registros.candidaturas ?? 0).toLocaleString('pt-BR')} candidaturas ativas
					</p>
				</div>
			</div>

			<!-- Card 3: Quadro Societário QSA -->
			<div class="p-5 bg-slate-800/70 border border-slate-700/60 rounded-xl flex items-center gap-4">
				<div class="w-12 h-12 rounded-lg bg-teal-500/10 border border-teal-500/30 flex items-center justify-center text-teal-400">
					<svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" />
					</svg>
				</div>
				<div>
					<p class="text-xs font-medium text-slate-400 uppercase tracking-wider">Sócios & Empresas QSA</p>
					<p class="text-2xl font-bold text-white mt-0.5">
						{#if loadingStatus && !status}
							<span class="text-slate-500 text-lg">...</span>
						{:else}
							{(status?.total_registros.empresas_qsa ?? 0).toLocaleString('pt-BR')}
						{/if}
					</p>
					<p class="text-xs text-slate-400">
						{(status?.total_registros.registros_profissionais ?? 0).toLocaleString('pt-BR')} registros OAB / CNA
					</p>
				</div>
			</div>

			<!-- Card 4: Receitas Eleitorais -->
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
						{(status?.total_registros.despesas_campanha ?? 0).toLocaleString('pt-BR')} despesas TSE
					</p>
				</div>
			</div>

			<!-- Card 5: Despesas CEAP -->
			<div class="p-5 bg-slate-800/70 border border-slate-700/60 rounded-xl flex items-center gap-4">
				<div class="w-12 h-12 rounded-lg bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400">
					<svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
					</svg>
				</div>
				<div>
					<p class="text-xs font-medium text-slate-400 uppercase tracking-wider">Cotas CEAP (Câmara)</p>
					<p class="text-2xl font-bold text-white mt-0.5">
						{#if loadingStatus && !status}
							<span class="text-slate-500 text-lg">...</span>
						{:else}
							{(status?.total_registros.despesas_parlamentares ?? 0).toLocaleString('pt-BR')}
						{/if}
					</p>
					<p class="text-xs text-slate-400">
						{(status?.total_registros.contratos_publicos ?? 0).toLocaleString('pt-BR')} contratos PNCP
					</p>
				</div>
			</div>

			<!-- Card 6: Contratos & Anomalias -->
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
						anomalias detectadas no auditor
					</p>
				</div>
			</div>
		</div>
	</div>

	<!-- Seção: Identidade Visual e Personalização do Favicon e Ícone -->
	<div>
		<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 mb-4">
			<div>
				<h2 class="text-base font-semibold text-slate-200 flex items-center gap-2">
					<span class="w-2 h-2 rounded-full bg-emerald-400"></span>
					Identidade Visual & Ícones da Aplicação
				</h2>
				<p class="text-xs text-slate-400 mt-0.5">Altere o favicon do navegador e o logo/marca exibido no topo da aplicação</p>
			</div>
			{#if identidade?.tem_icone_customizado || identidade?.tem_favicon_customizado}
				<span class="px-2.5 py-1 text-xs font-medium rounded-full bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 flex items-center gap-1.5 w-fit">
					<span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
					Ícone Personalizado Ativo
				</span>
			{:else}
				<span class="px-2.5 py-1 text-xs font-medium rounded-full bg-slate-800 border border-slate-700 text-slate-400 w-fit">
					Ícone Padrão do Radar
				</span>
			{/if}
		</div>

		<div class="bg-slate-800/40 border border-slate-700/60 rounded-xl p-6 space-y-6">
			<div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
				<!-- Preview Atual -->
				<div class="p-5 bg-slate-900/60 border border-slate-800 rounded-xl space-y-4">
					<span class="text-xs font-semibold text-slate-300 uppercase tracking-wider block">Pré-visualização</span>

					<div class="flex items-center gap-4">
						<div class="text-center">
							<div class="w-16 h-16 rounded-xl bg-slate-950 border border-slate-700 flex items-center justify-center p-2 mx-auto overflow-hidden shadow-inner">
								<img
									src={previewIconeUrl || `/api/v1/config/icone?v=${iconTimestamp}`}
									alt="Ícone Aplicação"
									class="w-full h-full object-contain"
								/>
							</div>
							<span class="text-[11px] text-slate-400 mt-1.5 block">Barra Superior</span>
						</div>

						<!-- Simulador de Aba de Navegador -->
						<div class="flex-1">
							<div class="bg-slate-950 border border-slate-800 rounded-lg p-2.5 shadow">
								<div class="flex items-center gap-2 px-2 py-1 bg-slate-900 rounded border border-slate-800 max-w-[220px]">
									<img
										src={previewIconeUrl || `/api/v1/config/favicon?v=${iconTimestamp}`}
										alt="Favicon"
										class="w-4 h-4 object-contain flex-shrink-0"
									/>
									<span class="text-xs font-medium text-slate-300 truncate">Radar Cívico</span>
								</div>
							</div>
							<span class="text-[11px] text-slate-400 mt-1.5 block">Aba do Navegador (Favicon)</span>
						</div>
					</div>

					{#if previewIconeUrl}
						<div class="p-2.5 bg-amber-500/10 border border-amber-500/30 rounded-lg text-amber-300 text-xs flex items-center justify-between">
							<span>Nova imagem selecionada</span>
							<button
								type="button"
								on:click={() => {
									if (previewIconeUrl) URL.revokeObjectURL(previewIconeUrl);
									previewIconeUrl = null;
									arquivoIcone = null;
								}}
								class="text-amber-400 hover:text-white underline text-[11px]"
							>
								Cancelar
							</button>
						</div>
					{/if}
				</div>

				<!-- Controles de Upload e Destino -->
				<div class="lg:col-span-2 space-y-4">
					<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
						<div>
							<label for="alvo-icone" class="block text-xs font-medium text-slate-300 mb-1.5">
								Onde aplicar o novo ícone:
							</label>
							<select
								id="alvo-icone"
								bind:value={alvoIcone}
								class="w-full bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:ring-emerald-500 focus:border-emerald-500"
							>
								<option value="ambos">Aplicar a Ambos (Ícone & Favicon)</option>
								<option value="icone">Apenas Ícone da Barra Superior</option>
								<option value="favicon">Apenas Favicon da Aba do Navegador</option>
							</select>
							<p class="text-[11px] text-slate-500 mt-1">Formato ideal: PNG transparente, SVG ou ICO quadrado</p>
						</div>

						<div>
							<span class="block text-xs font-medium text-slate-300 mb-1.5">Selecionar Arquivo de Imagem:</span>
							<input
								type="file"
								id="icon-file-input"
								accept=".png,.ico,.svg,.jpg,.jpeg,.webp"
								on:change={handleIconSelect}
								class="hidden"
							/>
							<label
								for="icon-file-input"
								class="cursor-pointer w-full flex items-center justify-center gap-2 px-3 py-2 bg-slate-900 hover:bg-slate-800 text-slate-300 hover:text-white text-xs font-medium rounded-lg border border-slate-700 transition-colors"
							>
								<svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
									<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
								</svg>
								<span class="truncate">{arquivoIcone ? arquivoIcone.name : 'Escolher Imagem (.png, .svg, .ico)'}</span>
							</label>
						</div>
					</div>

					<!-- Botões de Ação -->
					<div class="flex flex-wrap items-center gap-3 pt-2">
						<button
							type="button"
							on:click={salvarIcone}
							disabled={!arquivoIcone || salvandoIcone}
							class="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50 flex items-center gap-2"
						>
							{#if salvandoIcone}
								<svg class="w-3.5 h-3.5 animate-spin text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
									<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
								</svg>
								<span>Salvando...</span>
							{:else}
								<span>Salvar Alterações de Ícone</span>
							{/if}
						</button>

						<button
							type="button"
							on:click={restaurarIconePadrao}
							disabled={resetandoIcone}
							class="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white text-xs font-medium rounded-lg border border-slate-700 transition-colors disabled:opacity-50"
						>
							{resetandoIcone ? 'Restaurando...' : 'Restaurar Padrão do Radar'}
						</button>
					</div>

					{#if msgIconeSucesso}
						<div class="p-3 bg-emerald-500/10 border border-emerald-500/30 rounded-lg text-emerald-300 text-xs flex items-center gap-2">
							<span>✅</span>
							<span>{msgIconeSucesso}</span>
						</div>
					{/if}

					{#if erroIcone}
						<div class="p-3 bg-rose-500/10 border border-rose-500/30 rounded-lg text-rose-300 text-xs flex items-center gap-2">
							<span>❌</span>
							<span>{erroIcone}</span>
						</div>
					{/if}
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

				<!-- Toggle Descoberta CKAN -->
				<div class="p-3 bg-slate-900/60 border border-slate-800 rounded-lg space-y-1.5">
					<label class="flex items-center gap-2 cursor-pointer select-none">
						<input
							type="checkbox"
							bind:checked={descobrirViaCkanTse}
							class="rounded bg-slate-950 border-slate-700 text-emerald-500 focus:ring-emerald-500 w-4 h-4 cursor-pointer"
						/>
						<span class="text-xs font-medium text-slate-200">Descoberta Autônoma via API CKAN</span>
						<span class="text-[10px] uppercase font-bold tracking-wider px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400 border border-blue-500/30">dadosabertos.tse.jus.br</span>
					</label>
					<p class="text-[11px] text-slate-400 pl-6 leading-relaxed">
						Localiza pacotes oficiais via API do TSE e prioriza arquivos consolidados nacionais (<code class="text-emerald-400 font-mono text-[10px]">_BRASIL.csv</code>) com streaming e batch insert atômico.
					</p>
				</div>

				<div class="flex flex-wrap items-center gap-3 pt-1">
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
						type="button"
						on:click={verificarDisponibilidadeTse}
						disabled={verificandoTse}
						class="px-3 py-1.5 bg-slate-700/80 hover:bg-slate-700 text-slate-300 text-xs font-medium rounded-lg border border-slate-600 transition-colors disabled:opacity-50"
					>
						{verificandoTse ? 'Verificando...' : 'Checar Disponibilidade'}
					</button>

					<button
						type="button"
						on:click={dispararSincronizacaoTse}
						disabled={sincronizandoTse}
						class="px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50 flex items-center gap-1.5"
					>
						{#if sincronizandoTse}
							<svg class="w-3.5 h-3.5 animate-spin text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
							</svg>
							<span>Despachando...</span>
						{:else}
							<span>Sincronizar Dados</span>
						{/if}
					</button>
				</div>

				{#if msgTseVerificacao}
					<p class="text-xs text-slate-300 bg-slate-900/60 p-2.5 rounded-lg border border-slate-700/50">
						ℹ️ {msgTseVerificacao}
					</p>
				{/if}
			</div>

			<!-- Fonte 2: CEAP (Câmara dos Deputados - Ingestão Dual) -->
			<div class="p-6 bg-slate-800/50 border border-slate-700/60 rounded-xl space-y-4">
				<div class="flex items-center justify-between">
					<div class="flex items-center gap-3">
						<div class="w-9 h-9 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 font-bold text-xs">
							CEAP
						</div>
						<div>
							<h3 class="font-medium text-white flex items-center gap-2">
								Câmara dos Deputados (CEAP)
								<span class="text-[10px] font-normal px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-300 border border-emerald-500/30">
									Ingestão Dual
								</span>
							</h3>
							<p class="text-xs text-slate-400">Reembolsos parlamentares, notas fiscais e limites físicos de combustível</p>
						</div>
					</div>
				</div>

				<!-- Seletor de Modo de Ingestão: BULK vs API REST -->
				<div class="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-1">
					<button
						type="button"
						on:click={() => (modoCeap = 'BULK')}
						class="flex items-start gap-3 p-3 rounded-lg border text-left transition-all {modoCeap === 'BULK' ? 'bg-emerald-950/40 border-emerald-500/60 text-white shadow-sm' : 'bg-slate-900/60 border-slate-700/70 text-slate-400 hover:border-slate-600'}"
					>
						<span class="text-lg">📦</span>
						<div>
							<div class="text-xs font-semibold {modoCeap === 'BULK' ? 'text-emerald-400' : 'text-slate-300'}">Dump Anual (Zip/CSV)</div>
							<p class="text-[11px] text-slate-400 mt-0.5 leading-snug">Carga em massa do consolidado anual direto dos arquivos estáticos da Câmara.</p>
						</div>
					</button>

					<button
						type="button"
						on:click={() => (modoCeap = 'API')}
						class="flex items-start gap-3 p-3 rounded-lg border text-left transition-all {modoCeap === 'API' ? 'bg-emerald-950/40 border-emerald-500/60 text-white shadow-sm' : 'bg-slate-900/60 border-slate-700/70 text-slate-400 hover:border-slate-600'}"
					>
						<span class="text-lg">🌐</span>
						<div>
							<div class="text-xs font-semibold {modoCeap === 'API' ? 'text-emerald-400' : 'text-slate-300'}">API REST v2 (HATEOAS)</div>
							<p class="text-[11px] text-slate-400 mt-0.5 leading-snug">Consulta incremental paginada via links <code class="text-emerald-300">rel=next</code> com rate-limiting.</p>
						</div>
					</button>
				</div>

				<div class="flex flex-wrap items-center gap-3 pt-1">
					<div class="flex items-center gap-2">
						<label for="ano-ceap" class="text-xs text-slate-400 font-medium">Ano Fiscal:</label>
						<select
							id="ano-ceap"
							bind:value={anoCeap}
							class="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-2.5 py-1.5 focus:ring-emerald-500 focus:border-emerald-500"
						>
							<option value={2026}>2026</option>
							<option value={2025}>2025</option>
							<option value={2024}>2024</option>
							<option value={2023}>2023</option>
							<option value={2022}>2022</option>
							<option value={2021}>2021</option>
							<option value={2020}>2020</option>
						</select>
					</div>

					{#if modoCeap === 'API'}
						<div class="flex items-center gap-2">
							<label for="deputados-ceap" class="text-xs text-slate-400 font-medium">IDs Deputados:</label>
							<input
								id="deputados-ceap"
								type="text"
								bind:value={idsDeputadosCeap}
								placeholder="Ex: 204523, 204524 (opcional)"
								class="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-2.5 py-1.5 w-48 focus:ring-emerald-500 focus:border-emerald-500 placeholder-slate-500"
							/>
						</div>

						<div class="flex items-center gap-2">
							<label for="max-pags-ceap" class="text-xs text-slate-400 font-medium">Páginas Máx:</label>
							<select
								id="max-pags-ceap"
								bind:value={maxPaginasCeap}
								class="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-2.5 py-1.5 focus:ring-emerald-500 focus:border-emerald-500"
							>
								<option value={1}>1 pág (~100 itens)</option>
								<option value={5}>5 págs (~500 itens)</option>
								<option value={10}>10 págs (~1000 itens)</option>
								<option value={50}>50 págs</option>
							</select>
						</div>
					{/if}

					<button
						on:click={dispararSincronizacaoCamara}
						disabled={sincronizandoCamara || activeJob?.status === 'PROCESSANDO'}
						class="px-4 py-1.5 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50 flex items-center gap-2 ml-auto"
					>
						{#if sincronizandoCamara}
							<svg class="animate-spin w-3.5 h-3.5" fill="none" viewBox="0 0 24 24">
								<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
								<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
							</svg>
							<span>Disparando...</span>
						{:else if modoCeap === 'BULK'}
							<span>Sincronizar Dump Anual</span>
						{:else}
							<span>Sincronizar via API REST</span>
						{/if}
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

			{#if importProgress && (importProgress.is_running || importProgress.records_processed > 0)}
				<div class="grid grid-cols-2 sm:grid-cols-4 gap-3 bg-slate-900/80 p-3 rounded-lg border border-slate-700/60 text-xs">
					<div>
						<span class="text-slate-400 block text-[11px]">Arquivo Atual</span>
						<span class="text-slate-200 font-medium truncate block" title={importProgress.current_file}>
							{importProgress.current_file || '-'}
						</span>
					</div>
					<div>
						<span class="text-slate-400 block text-[11px]">Pacotes Processados</span>
						<span class="text-emerald-400 font-mono font-semibold">
							{importProgress.files_processed} / {importProgress.total_files}
						</span>
					</div>
					<div>
						<span class="text-slate-400 block text-[11px]">Registros Salvos (SQLite)</span>
						<span class="text-cyan-400 font-mono font-semibold">
							{importProgress.records_processed.toLocaleString('pt-BR')}
						</span>
					</div>
					<div>
						<span class="text-slate-400 block text-[11px]">Tempo Decorrido</span>
						<span class="text-amber-400 font-mono font-semibold">
							{importProgress.elapsed_seconds}s
						</span>
					</div>
				</div>
			{/if}

			<!-- Terminal de Logs -->
			<div class="bg-slate-950 p-4 rounded-lg border border-slate-800 font-mono text-xs text-slate-300 space-y-1 max-h-48 overflow-y-auto">
				{#each activeJob.logs as log}
					<p class="leading-relaxed">{log}</p>
				{/each}
			</div>
		</div>
	{/if}

	<!-- Seção: Monitoramento e Controle Unificado de Fontes Públicas (Framework SourceImporter) -->
	<div>
		<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 mb-4">
			<h2 class="text-base font-semibold text-slate-200 flex items-center gap-2">
				<span class="w-2 h-2 rounded-full bg-indigo-400"></span>
				Monitoramento & Controle Unificado de Fontes Públicas
			</h2>
			<button
				on:click={carregarImporters}
				class="self-start sm:self-auto px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-xs text-slate-300 font-medium rounded-lg border border-slate-700 flex items-center gap-1.5 transition-colors"
			>
				<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
				</svg>
				Atualizar Status
			</button>
		</div>

		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
			{#each importers as imp}
				<div class="p-5 bg-slate-800/60 border border-slate-700/70 rounded-xl space-y-3.5 flex flex-col justify-between">
					<div class="space-y-3">
						<div class="flex items-start justify-between gap-2">
							<div>
								<div class="flex items-center gap-2">
									<span class="text-sm font-bold text-white">{imp.name}</span>
									<span class="px-1.5 py-0.5 text-[10px] font-mono rounded bg-slate-900 text-slate-400 border border-slate-700">
										{imp.id}
									</span>
								</div>
								<p class="text-xs text-slate-400 mt-1 leading-snug">{imp.description}</p>
							</div>
							<span class="text-[11px] font-semibold px-2 py-0.5 rounded uppercase shrink-0
								{imp.stage === 'CONCLUIDO' ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30' :
								 imp.stage === 'ERRO' ? 'bg-rose-500/20 text-rose-400 border border-rose-500/30' :
								 imp.stage === 'CANCELADO' ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30' :
								 imp.is_running ? 'bg-blue-500/20 text-blue-400 border border-blue-500/30 animate-pulse' :
								 'bg-slate-700/40 text-slate-400 border border-slate-700'}">
								{imp.stage}
							</span>
						</div>

						<!-- Barra de Progresso do Importer -->
						<div class="space-y-1">
							<div class="flex justify-between text-[11px] font-mono text-slate-400">
								<span>Progresso</span>
								<span>{Math.round(imp.percentage)}%</span>
							</div>
							<div class="w-full bg-slate-950 rounded-full h-2 overflow-hidden border border-slate-800">
								<div
									class="h-2 rounded-full transition-all duration-300
										{imp.stage === 'CONCLUIDO' ? 'bg-emerald-500' :
										 imp.stage === 'ERRO' ? 'bg-rose-500' :
										 imp.stage === 'CANCELADO' ? 'bg-orange-500' : 'bg-indigo-500'}"
									style="width: {imp.percentage}%"
								></div>
							</div>
						</div>

						<!-- Métricas Detalhadas -->
						<div class="grid grid-cols-2 gap-2 p-2.5 bg-slate-900/60 rounded-lg border border-slate-800 text-[11px]">
							<div>
								<span class="text-slate-400 block text-[10px]">Arquivos</span>
								<span class="text-slate-200 font-semibold">{imp.files_processed} / {imp.total_files}</span>
							</div>
							<div>
								<span class="text-slate-400 block text-[10px]">Registros Salvos</span>
								<span class="text-cyan-400 font-semibold font-mono">{imp.records_processed.toLocaleString('pt-BR')}</span>
							</div>
							<div class="col-span-2">
								<span class="text-slate-400 block text-[10px]">Status / Mensagem</span>
								<span class="text-slate-300 truncate block text-[11px]" title={imp.message}>
									{imp.message || 'Pronto para execução'}
								</span>
							</div>
						</div>
					</div>

					<!-- Botões de Ação -->
					<div class="pt-2 border-t border-slate-700/40 flex items-center justify-between">
						<span class="text-[11px] text-slate-400 font-mono">
							Tempo: {imp.elapsed_seconds}s
						</span>

						{#if imp.is_running}
							<button
								on:click={() => cancelarImporter(imp.id)}
								disabled={acaoImporterId === imp.id}
								class="px-3 py-1.5 bg-rose-600/90 hover:bg-rose-600 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50 flex items-center gap-1"
							>
								{#if acaoImporterId === imp.id}
									<span class="animate-spin text-xs">⟳</span>
								{/if}
								Cancelar
							</button>
						{:else}
							<button
								on:click={() => iniciarImporter(imp.id)}
								disabled={acaoImporterId === imp.id}
								class="px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50 flex items-center gap-1"
							>
								{#if acaoImporterId === imp.id}
									<span class="animate-spin text-xs">⟳</span>
								{/if}
								Iniciar Carga
							</button>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	</div>

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
					<label for="tipo-doc-upload" class="text-xs text-slate-400">Tipo de Documento:</label>
					<select
						id="tipo-doc-upload"
						bind:value={tipoDocumentoUpload}
						class="bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-1.5 focus:ring-emerald-500 focus:border-emerald-500"
					>
						<option value="AUTO">Detecção Automática de Cabeçalho (Recomendado)</option>
						<option value="RECEITA_QSA">Receita Federal - Quadro Societário (QSA / Sócios / Holdings)</option>
						<option value="CONSELHOS_OAB">Conselhos de Classe / OAB (CNA)</option>
						<option value="DIARIOS_OFICIAIS">Diários Oficiais / Atos de Nomeação</option>
						<option value="PNCP_CONTRATOS">PNCP - Contratos e Licitações Públicas</option>
						<option value="CEAP_NOTAS">Câmara dos Deputados - Cotas CEAP</option>
						<option value="TSE_RECEITAS">TSE - Prestação de Contas (Receitas de Campanha)</option>
						<option value="TSE_DESPESAS">TSE - Prestação de Contas (Despesas de Campanha)</option>
						<option value="TSE_CANDIDATOS">TSE - Candidatos (consulta_cand)</option>
						<option value="AUXILIO_EMERGENCIAL">Auxílio Emergencial / Benefícios (CGU / Brasil.IO)</option>
					</select>
				</div>
			</div>

			<!-- Guia de Cabeçalhos Suportados Toggle -->
			<div class="border-t border-slate-700/50 pt-3">
				<button
					type="button"
					on:click={() => (mostrarGuiaCabecalhos = !mostrarGuiaCabecalhos)}
					class="text-xs text-emerald-400 hover:text-emerald-300 font-medium flex items-center gap-1.5"
				>
					<svg class="w-3.5 h-3.5 transition-transform duration-200 {mostrarGuiaCabecalhos ? 'rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
					</svg>
					<span>{mostrarGuiaCabecalhos ? 'Ocultar Dicionário de Cabeçalhos Suportados' : 'Ver Dicionário de Cabeçalhos Suportados (Holdings, QSA, OAB, PNCP, CEAP, TSE, Auxílio)'}</span>
				</button>

				{#if mostrarGuiaCabecalhos}
					<div class="mt-3 p-4 bg-slate-900/80 border border-slate-800 rounded-lg text-xs space-y-3 animate-fade-in">
						<p class="text-slate-300 font-medium">O motor de ingestão em streaming detecta automaticamente delimitadores (<code class="text-emerald-400">;</code> ou <code class="text-emerald-400">,</code>) e aceita as seguintes colunas:</p>
						<div class="grid grid-cols-1 md:grid-cols-2 gap-3">
							<div class="p-3 bg-slate-950/60 rounded border border-slate-800/80 space-y-1">
								<span class="font-semibold text-emerald-400 block">Receita Federal (QSA / Sócios / Holdings):</span>
								<p class="text-slate-400 text-[11px]">Colunas aceitas: <code class="text-slate-300">CNPJ_BASICO</code> (ou <code class="text-slate-300">CNPJ</code> formatado/14 dígitos), <code class="text-slate-300">RAZAO_SOCIAL</code> (ou <code class="text-slate-300">EMPRESA</code>), <code class="text-slate-300">NOME_SOCIO</code> (ou <code class="text-slate-300">HOLDING</code> / <code class="text-slate-300">CONTROLADORA</code>), <code class="text-slate-300">CPF_CNPJ_SOCIO</code> (ou <code class="text-slate-300">CNPJ_HOLDING</code>), <code class="text-slate-300">QUALIFICACAO_SOCIO</code></p>
								<p class="text-[10px] text-slate-500">Tabela de destino: <span class="font-mono text-indigo-300">empresas_qsa</span> (suporta holdings PJ e formatação automática)</p>
							</div>

							<div class="p-3 bg-slate-950/60 rounded border border-slate-800/80 space-y-1">
								<span class="font-semibold text-cyan-400 block">Conselhos Profissionais / OAB:</span>
								<p class="text-slate-400 text-[11px]">Colunas aceitas: <code class="text-slate-300">PESSOA_NOME</code>, <code class="text-slate-300">CPF_MASCARADO</code>, <code class="text-slate-300">ORGAO_EMISSOR</code>, <code class="text-slate-300">NUMERO_REGISTRO</code>, <code class="text-slate-300">SECCIONAL_UF</code>, <code class="text-slate-300">SITUACAO_REGISTRO</code></p>
								<p class="text-[10px] text-slate-500">Tabela de destino: <span class="font-mono text-indigo-300">registros_profissionais</span></p>
							</div>

							<div class="p-3 bg-slate-950/60 rounded border border-slate-800/80 space-y-1">
								<span class="font-semibold text-purple-400 block">PNCP (Contratos Públicos):</span>
								<p class="text-slate-400 text-[11px]">Colunas aceitas: <code class="text-slate-300">ORGAO_CONTRATANTE</code>, <code class="text-slate-300">FORNECEDOR_CNPJ</code>, <code class="text-slate-300">VALOR_CONTRATADO</code>, <code class="text-slate-300">OBJETO</code>, <code class="text-slate-300">DATA_ASSINATURA</code></p>
								<p class="text-[10px] text-slate-500">Tabela de destino: <span class="font-mono text-indigo-300">contratos_publicos</span></p>
							</div>

							<div class="p-3 bg-slate-950/60 rounded border border-slate-800/80 space-y-1">
								<span class="font-semibold text-amber-400 block">Câmara dos Deputados (CEAP):</span>
								<p class="text-slate-400 text-[11px]">Colunas aceitas: <code class="text-slate-300">TXNOMEPARLAMENTAR</code>, <code class="text-slate-300">CPF</code>, <code class="text-slate-300">DATANF</code>, <code class="text-slate-300">NUMDOCUMENTO</code>, <code class="text-slate-300">VLRLIQUIDO</code>, <code class="text-slate-300">FORNECEDOR</code>, <code class="text-slate-300">CNPJCPF</code></p>
								<p class="text-[10px] text-slate-500">Tabela de destino: <span class="font-mono text-indigo-300">despesas_parlamentares</span></p>
							</div>

							<div class="p-3 bg-slate-950/60 rounded border border-slate-800/80 space-y-1">
								<span class="font-semibold text-rose-400 block">TSE (Prestação de Contas):</span>
								<p class="text-slate-400 text-[11px]">Receitas: <code class="text-slate-300">NR_CPF_CNPJ_DOADOR</code>, <code class="text-slate-300">NM_DOADOR</code>, <code class="text-slate-300">VR_RECEITA</code>, <code class="text-slate-300">DT_RECEITA</code></p>
								<p class="text-slate-400 text-[11px]">Despesas: <code class="text-slate-300">NR_CPF_CNPJ_FORNECEDOR</code>, <code class="text-slate-300">NM_FORNECEDOR</code>, <code class="text-slate-300">VR_DESPESA</code>, <code class="text-slate-300">DT_DESPESA</code></p>
							</div>

							<div class="p-3 bg-slate-950/60 rounded border border-slate-800/80 space-y-1">
								<span class="font-semibold text-rose-300 block">Auxílio Emergencial (CGU / Brasil.IO):</span>
								<p class="text-slate-400 text-[11px]">Colunas aceitas: <code class="text-slate-300">CPF_BENEFICIARIO</code>, <code class="text-slate-300">NOME_BENEFICIARIO</code>, <code class="text-slate-300">VALOR_BENEFICIO</code>, <code class="text-slate-300">MES_DISPONIBILIZACAO</code>, <code class="text-slate-300">PARCELA</code>, <code class="text-slate-300">UF</code>, <code class="text-slate-300">MUNICIPIO</code></p>
								<p class="text-[10px] text-slate-500">Tabela de destino: <span class="font-mono text-indigo-300">beneficios_emergenciais</span></p>
							</div>

							<div class="p-3 bg-slate-950/60 rounded border border-slate-800/80 space-y-1">
								<span class="font-semibold text-blue-400 block">Querido Diário / Nomeações:</span>
								<p class="text-slate-400 text-[11px]">Colunas aceitas: <code class="text-slate-300">DOADOR_CPF_CNPJ</code>, <code class="text-slate-300">TERMO_PESQUISADO</code>, <code class="text-slate-300">MUNICIPIO_UF</code></p>
								<p class="text-[10px] text-slate-500">Tabela de destino: <span class="font-mono text-indigo-300">cache_consultas_diario</span></p>
							</div>
						</div>
					</div>
				{/if}
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

	<!-- Rodapé Informativo da Tela de Configurações -->
	<footer class="mt-8 pt-6 border-t border-slate-800/80 flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-slate-500">
		<div class="flex items-center gap-2">
			<span class="inline-block w-2 h-2 rounded-full bg-indigo-400"></span>
			<span>Painel Administrativo &bull; Radar Cívico</span>
		</div>
		<div class="flex items-center gap-3">
			<span class="text-slate-400">Versão da Plataforma:</span>
			<span
				class="inline-flex items-center gap-1.5 px-3 py-1 rounded-lg bg-slate-800/90 border border-slate-700/80 font-mono text-xs font-semibold text-emerald-400 shadow-sm cursor-help hover:border-emerald-500/50 transition-colors"
				title={`Commit Git: ${$versionStore.commit}`}
				aria-label={`Versão ${$versionStore.version}, commit ${$versionStore.commit}`}
			>
				<span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
				<span>{$versionStore.version}</span>
				<span class="text-slate-400 text-[11px] font-normal">({$versionStore.commit})</span>
			</span>
		</div>
	</footer>
</div>
