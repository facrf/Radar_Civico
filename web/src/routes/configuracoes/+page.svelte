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
		cargos_autoridades?: number;
		emendas_parlamentares?: number;
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
		status: 'PENDENTE' | 'PROCESSANDO' | 'CONCLUIDO' | 'PARCIAL' | 'INTERROMPIDO' | 'ERRO';
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

	// Parâmetros do Motor de Auditoria
	interface ParametrosAuditoria {
		limite_combustivel_litros: number;
		preco_combustivel_referencia: number;
		sobrepreco_combustivel_percentual: number;
		estimar_volume_combustivel: boolean;
		referencia_combustivel_periodo: string;
		referencia_combustivel_local: string;
		referencia_combustivel_url: string | null;
		janela_triangulacao_dias: number;
		concentracao_fornecedor_percentual: number;
	}

	let auditRules: ParametrosAuditoria = {
		limite_combustivel_litros: 250,
		preco_combustivel_referencia: 5.8,
		sobrepreco_combustivel_percentual: 50,
		estimar_volume_combustivel: true,
		referencia_combustivel_periodo: 'Não informado',
		referencia_combustivel_local: 'Não informado',
		referencia_combustivel_url: null,
		janela_triangulacao_dias: 180,
		concentracao_fornecedor_percentual: 60
	};
	let carregandoAuditRules = false;
	let salvandoAuditRules = false;
	let msgAuditRulesSucesso: string | null = null;
	let erroAuditRules: string | null = null;

	async function carregarAuditRules() {
		carregandoAuditRules = true;
		erroAuditRules = null;
		try {
			const res = await fetch('/api/settings/audit-rules');
			if (res.ok) {
				const data = await res.json();
				auditRules = {
					limite_combustivel_litros: Number(data.limite_combustivel_litros) || 250,
					preco_combustivel_referencia: Number(data.preco_combustivel_referencia ?? 5.8),
					sobrepreco_combustivel_percentual: Number(data.sobrepreco_combustivel_percentual ?? 50),
					estimar_volume_combustivel: data.estimar_volume_combustivel ?? true,
					referencia_combustivel_periodo: data.referencia_combustivel_periodo ?? 'Não informado',
					referencia_combustivel_local: data.referencia_combustivel_local ?? 'Não informado',
					referencia_combustivel_url: data.referencia_combustivel_url ?? null,
					janela_triangulacao_dias: Number(data.janela_triangulacao_dias) || 180,
					concentracao_fornecedor_percentual: Number(data.concentracao_fornecedor_percentual ?? 60)
				};
			}
		} catch (e: any) {
			console.error('Erro ao buscar parâmetros de auditoria:', e);
		} finally {
			carregandoAuditRules = false;
		}
	}

	async function salvarAuditRules() {
		if (isNaN(auditRules.limite_combustivel_litros) || auditRules.limite_combustivel_litros < 1 || auditRules.limite_combustivel_litros > 10000) {
			erroAuditRules = 'Limite de combustível deve estar entre 1 L e 10.000 L.';
			return;
		}
		if (isNaN(auditRules.janela_triangulacao_dias) || auditRules.janela_triangulacao_dias < 1 || auditRules.janela_triangulacao_dias > 730) {
			erroAuditRules = 'Janela de triangulação deve estar entre 1 e 730 dias.';
			return;
		}
		if (isNaN(auditRules.concentracao_fornecedor_percentual) || auditRules.concentracao_fornecedor_percentual < 0 || auditRules.concentracao_fornecedor_percentual > 100) {
			erroAuditRules = 'Concentração de fornecedor deve estar entre 0% e 100%.';
			return;
		}

		if (!Number.isFinite(auditRules.preco_combustivel_referencia) || auditRules.preco_combustivel_referencia < 0.01 || auditRules.preco_combustivel_referencia > 100) {
			erroAuditRules = 'Preço de referência deve estar entre R$ 0,01 e R$ 100,00 por litro.';
			return;
		}
		if (!Number.isFinite(auditRules.sobrepreco_combustivel_percentual) || auditRules.sobrepreco_combustivel_percentual < 0 || auditRules.sobrepreco_combustivel_percentual > 1000) {
			erroAuditRules = 'Margem de sobrepreço deve estar entre 0% e 1.000%.';
			return;
		}

		salvandoAuditRules = true;
		erroAuditRules = null;
		msgAuditRulesSucesso = null;

		try {
			const res = await fetch('/api/settings/audit-rules', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ ...auditRules, referencia_combustivel_url: auditRules.referencia_combustivel_url?.trim() || null })
			});

			if (!res.ok) {
				const err = await res.json();
				throw new Error(err.mensagem || err.erro || 'Falha ao salvar parâmetros do motor de auditoria');
			}

			const data = await res.json();
			if (data.parametros) {
				auditRules = {
					limite_combustivel_litros: Number(data.parametros.limite_combustivel_litros),
					preco_combustivel_referencia: Number(data.parametros.preco_combustivel_referencia ?? 5.8),
					sobrepreco_combustivel_percentual: Number(data.parametros.sobrepreco_combustivel_percentual ?? 50),
					estimar_volume_combustivel: data.parametros.estimar_volume_combustivel ?? true,
					referencia_combustivel_periodo: data.parametros.referencia_combustivel_periodo ?? 'Não informado',
					referencia_combustivel_local: data.parametros.referencia_combustivel_local ?? 'Não informado',
					referencia_combustivel_url: data.parametros.referencia_combustivel_url ?? null,
					janela_triangulacao_dias: Number(data.parametros.janela_triangulacao_dias),
					concentracao_fornecedor_percentual: Number(data.parametros.concentracao_fornecedor_percentual)
				};
			}

			msgAuditRulesSucesso = 'Parâmetros do motor de auditoria salvos com sucesso! Regras aplicadas dinamicamente.';
			window.dispatchEvent(new CustomEvent('radar-alertas-updated'));
			await carregarStatus();

			setTimeout(() => {
				msgAuditRulesSucesso = null;
			}, 6000);
		} catch (e: any) {
			erroAuditRules = e.message || 'Erro ao persistir parâmetros';
		} finally {
			salvandoAuditRules = false;
		}
	}

	async function restaurarPadroesAuditRules() {
		auditRules = {
			limite_combustivel_litros: 250,
			preco_combustivel_referencia: 5.8,
			sobrepreco_combustivel_percentual: 50,
			estimar_volume_combustivel: true,
			referencia_combustivel_periodo: 'Não informado',
			referencia_combustivel_local: 'Não informado',
			referencia_combustivel_url: null,
			janela_triangulacao_dias: 180,
			concentracao_fornecedor_percentual: 60
		};
		await salvarAuditRules();
	}

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

	// Sincronização Unificada (Sincronizar Tudo)
	let sincronizandoTudo = false;
	let modalSincronizarTudo = false;
	let incluirTseTudo = true;
	let incluirCamaraTudo = true;
	let incluirReceitaTudo = true;
	let incluirAutoridadesTudo = true;
	let incluirAuditoriaTudo = true;
	let sincronizandoAutoridades = false;
	let sincronizandoDiarios = false;
	let termoDiario = '';
	let municipioDiario = '';

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
		stage: 'IDLE' | 'CONECTANDO' | 'BAIXANDO' | 'DESCOMPACTANDO' | 'PROCESSANDO' | 'FINALIZANDO' | 'CONCLUIDO' | 'CANCELADO' | 'INTERROMPIDO' | 'ERRO';
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
		{ id: 'emendas_parlamentares', nome: 'Emendas Parlamentares (SIOP/Transparência)' },
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

	async function dispararSincronizarAutoridades() {
		sincronizandoAutoridades = true;
		try {
			const res = await fetch('/api/v1/config/sincronizar/autoridades', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' }
			});

			if (!res.ok) {
				const err = await res.json();
				throw new Error(err.erro || 'Falha ao sincronizar autoridades de cúpula');
			}

			const data = await res.json();
			iniciarPollingJob(data.job_id);
		} catch (e: any) {
			alert(`Erro na sincronização de autoridades: ${e.message}`);
		} finally {
			sincronizandoAutoridades = false;
		}
	}

	async function dispararSincronizarDiarios() {
		sincronizandoDiarios = true;
		try {
			const res = await fetch('/api/v1/config/diarios/sincronizar', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					termo: termoDiario.trim() || undefined,
					municipio: municipioDiario.trim() || undefined,
					limite: 15
				})
			});

			if (!res.ok) {
				const err = await res.json();
				throw new Error(err.erro || 'Falha ao sincronizar diários municipais');
			}

			const data = await res.json();
			iniciarPollingJob(data.job_id);
		} catch (e: any) {
			alert(`Erro na sincronização do Querido Diário: ${e.message}`);
		} finally {
			sincronizandoDiarios = false;
		}
	}

	async function dispararSincronizarTudo() {
		sincronizandoTudo = true;
		modalSincronizarTudo = false;
		try {
			const res = await fetch('/api/v1/config/sincronizar-tudo', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					ano_eleitoral: anoTse,
					ano_fiscal: anoCeap,
					incluir_tse: incluirTseTudo,
					incluir_camara: incluirCamaraTudo,
					incluir_receita: incluirReceitaTudo,
					incluir_autoridades: incluirAutoridadesTudo,
					incluir_auditoria: incluirAuditoriaTudo
				})
			});

			if (!res.ok) {
				const err = await res.json();
				throw new Error(err.erro || 'Falha ao despachar sincronização unificada');
			}

			const data = await res.json();
			iniciarPollingJob(data.job_id);
		} catch (e: any) {
			alert(`Erro na sincronização unificada: ${e.message}`);
		} finally {
			sincronizandoTudo = false;
		}
	}

	let deduplicandoMassa = false;
	async function dispararDeduplicacaoMassa() {
		if (!confirm('Deseja iniciar o processo assíncrono de consolidação de identidades repetidas no banco?')) {
			return;
		}
		deduplicandoMassa = true;
		try {
			const res = await fetch('/api/v1/politicos/duplicados/mesclar-automatico-job?limite=10000', {
				method: 'POST'
			});
			if (!res.ok) {
				const err = await res.json();
				throw new Error(err.erro || 'Falha ao despachar deduplicação');
			}
			const data = await res.json();
			iniciarPollingJob(data.job_id);
		} catch (e: any) {
			alert(`Erro na deduplicação: ${e.message}`);
		} finally {
			deduplicandoMassa = false;
		}
	}

	function iniciarPollingJob(jobId: string) {
        try { localStorage.setItem('radar:lastJobId',jobId); } catch { /* Armazenamento do navegador pode estar desabilitado. */ }
		if (pollingInterval) clearInterval(pollingInterval);

		const verificar = async () => {
			try {
				const [resJob, resProg] = await Promise.all([
					fetch(`/api/v1/config/ingestao/status/${jobId}`),
					fetch('/api/import/status')
				]);

				if (resJob.ok) {
                    activeJob = await resJob.json();
                } else if (resJob.status === 404) {
                    try { localStorage.removeItem('radar:lastJobId'); } catch { /* Sem armazenamento local. */ }
                    if (pollingInterval) clearInterval(pollingInterval);
                    pollingInterval = null;
                }
				if (resProg.ok) {
					importProgress = await resProg.json();
				}

				if (activeJob?.status === 'CONCLUIDO' || activeJob?.status === 'ERRO' || activeJob?.status === 'PARCIAL' || activeJob?.status === 'INTERROMPIDO') {
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

	// Hot SQLite Snapshots & Backups
	interface BackupItem {
		nome_arquivo: string;
		tamanho_bytes: number;
		tamanho_formatado: string;
		criado_em: string;
		download_url: string;
	}

	let backups: BackupItem[] = [];
	let carregandoBackups = false;
	let criandoBackup = false;
	let msgBackup: string | null = null;
	let erroBackup: string | null = null;

	async function carregarBackups() {
		carregandoBackups = true;
		erroBackup = null;
		try {
			const res = await fetch('/api/v1/config/backups');
			if (res.ok) {
				backups = await res.json();
			}
		} catch (e: any) {
			erroBackup = 'Falha ao listar backups disponíveis';
		} finally {
			carregandoBackups = false;
		}
	}

	async function criarBackupQuente() {
		criandoBackup = true;
		msgBackup = null;
		erroBackup = null;
		try {
			const res = await fetch('/api/v1/config/backup', { method: 'POST' });
			const data = await res.json();
			if (res.ok && data.status === 'sucesso') {
				msgBackup = `Backup ${data.nome_arquivo} (${data.tamanho_formatado}) gerado com sucesso!`;
				await carregarBackups();
			} else {
				erroBackup = data.mensagem || 'Falha ao executar snapshot';
			}
		} catch (e: any) {
			erroBackup = e.message || 'Erro de rede ao criar backup';
		} finally {
			criandoBackup = false;
		}
	}

	// Webhooks
	let webhookUrl = '';
	let testandoWebhook = false;
	let resultadoWebhook: { sucesso: boolean; status_code?: number; mensagem: string } | null = null;

	async function testarWebhook() {
		if (!webhookUrl.trim()) return;
		testandoWebhook = true;
		resultadoWebhook = null;
		try {
			const res = await fetch('/api/v1/config/webhook/test', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ url: webhookUrl.trim() })
			});
			resultadoWebhook = await res.json();
		} catch (e: any) {
			resultadoWebhook = {
				sucesso: false,
				mensagem: `Erro de rede: ${e.message}`
			};
		} finally {
			testandoWebhook = false;
		}
	}

	// Sincronização do Motor de Auditoria
	let sincronizandoAuditoria = false;
	let msgSincAuditoria: string | null = null;

	async function sincronizarMotorAuditoria() {
		sincronizandoAuditoria = true;
		msgSincAuditoria = null;
		try {
			const res = await fetch('/api/auditoria/sincronizar', { method: 'POST' });
			const data = await res.json();
			if (res.ok && data.status === 'sucesso') {
				msgSincAuditoria = data.mensagem || `${data.novos_alertas} novos alertas sincronizados.`;
				carregarStatus();
			} else {
				msgSincAuditoria = data.mensagem || 'Falha na sincronização';
			}
		} catch (e: any) {
			msgSincAuditoria = `Erro: ${e.message}`;
		} finally {
			sincronizandoAuditoria = false;
		}
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
        try { const last=localStorage.getItem('radar:lastJobId'); if(last) iniciarPollingJob(last); } catch { /* Sem armazenamento local. */ }
		carregarStatus();
		carregarAuditRules();
		carregarIdentidade();
		carregarImporters();
		carregarBackups();
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
				type="button"
				on:click={() => (modalSincronizarTudo = true)}
				disabled={sincronizandoTudo || (activeJob?.fonte === 'SINCRONIZAR_TUDO' && activeJob?.status === 'PROCESSANDO')}
				class="inline-flex items-center gap-2 px-4 py-2 bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-white text-sm font-semibold rounded-lg shadow-sm shadow-emerald-950/40 transition-all disabled:opacity-50 cursor-pointer"
				title="Sincronizar todas as fontes oficiais (TSE, CEAP, QSA, OAB) com proteção anti-duplicação e auditoria"
			>
				{#if sincronizandoTudo || (activeJob?.fonte === 'SINCRONIZAR_TUDO' && activeJob?.status === 'PROCESSANDO')}
					<svg class="w-4 h-4 animate-spin text-white" fill="none" viewBox="0 0 24 24">
						<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
						<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
					</svg>
					<span>Sincronizando Tudo...</span>
				{:else}
					<svg class="w-4 h-4 text-emerald-100" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
					</svg>
					<span>Sincronizar Tudo</span>
				{/if}
			</button>

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
						{(status?.total_registros.candidaturas ?? 0).toLocaleString('pt-BR')} candidaturas • {(status?.total_registros.cargos_autoridades ?? 0).toLocaleString('pt-BR')} autoridades públicas
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
						{(status?.total_registros.contratos_publicos ?? 0).toLocaleString('pt-BR')} contratos PNCP • {(status?.total_registros.emendas_parlamentares ?? 0).toLocaleString('pt-BR')} emendas
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

	<!-- Seção: Parâmetros do Motor de Auditoria -->
	<div>
		<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 mb-4">
			<div>
				<h2 class="text-base font-semibold text-slate-200 flex items-center gap-2">
					<span class="w-2 h-2 rounded-full bg-amber-400"></span>
					Parâmetros do Motor de Auditoria
				</h2>
				<p class="text-xs text-slate-400 mt-0.5">
					Ajuste fino da sensibilidade das regras determinísticas para evitar falsos positivos
				</p>
			</div>
			<div class="flex items-center gap-2">
				{#if auditRules.limite_combustivel_litros === 250 && auditRules.janela_triangulacao_dias === 180 && auditRules.concentracao_fornecedor_percentual === 60 && auditRules.preco_combustivel_referencia === 5.8 && auditRules.sobrepreco_combustivel_percentual === 50 && auditRules.estimar_volume_combustivel && !auditRules.referencia_combustivel_url && auditRules.referencia_combustivel_periodo === 'Não informado' && auditRules.referencia_combustivel_local === 'Não informado'}
					<span class="px-2.5 py-1 text-xs font-medium rounded-full bg-slate-800 border border-slate-700 text-slate-400">
						Limiares Padrão de Fábrica
					</span>
				{:else}
					<span class="px-2.5 py-1 text-xs font-medium rounded-full bg-amber-500/10 border border-amber-500/30 text-amber-300 flex items-center gap-1.5">
						<span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse"></span>
						Limiares Customizados Ativos
					</span>
				{/if}
			</div>
		</div>

		<div class="bg-slate-800/40 border border-slate-700/60 rounded-xl p-6 space-y-6">
			<div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
				<!-- Parâmetro 1: Limite de Combustível -->
				<div class="p-5 bg-slate-900/60 border border-slate-800 rounded-xl flex flex-col justify-between space-y-4">
					<div class="space-y-2">
						<div class="flex items-center justify-between">
							<span class="text-xs font-semibold text-white flex items-center gap-1.5">
								<svg class="w-4 h-4 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
									<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
								</svg>
								Limite de Combustível
							</span>
							<span class="px-2 py-0.5 rounded text-xs font-mono font-bold bg-amber-500/10 border border-amber-500/30 text-amber-300">
								{auditRules.limite_combustivel_litros} L
							</span>
						</div>
						<p class="text-[11px] text-slate-400 leading-snug">
							Volume máximo por abastecimento na CEAP. Volumes acima deste limite geram indícios para conferência. Uma nota pode abranger vários veículos ou abastecimentos.
						</p>
					</div>

					<div class="space-y-3">
						<!-- Slider -->
						<div class="space-y-1">
							<input
								type="range"
								min="1"
								max="1000"
								step="1"
								bind:value={auditRules.limite_combustivel_litros}
								class="w-full accent-amber-500 cursor-pointer h-2 bg-slate-800 rounded-lg"
							/>
							<div class="flex justify-between text-[10px] text-slate-500 font-mono">
								<span>1 L</span>
								<span class="text-amber-400/80">Padrão: 250 L</span>
								<span>1.000 L+</span>
							</div>
						</div>

						<!-- Entrada Manual -->
						<div class="flex items-center gap-2">
							<label for="input-combustivel" class="text-[11px] text-slate-400 whitespace-nowrap">Entrada direta (1 a 10.000 L):</label>
							<div class="relative flex-1">
								<input
									id="input-combustivel"
									type="number"
									min="1"
									max="10000"
									step="1"
									bind:value={auditRules.limite_combustivel_litros}
									class="w-full bg-slate-950 border border-slate-700 text-slate-100 text-xs rounded-lg px-2.5 py-1.5 pr-8 focus:ring-amber-500 focus:border-amber-500 font-mono"
								/>
								<span class="absolute right-2.5 top-1.5 text-xs text-slate-500 pointer-events-none">L</span>
							</div>
						</div>

						<!-- Presets rápidos -->
						<div class="flex items-center gap-1.5 pt-1">
							<span class="text-[10px] text-slate-500">Atalhos:</span>
							<button
								type="button"
								on:click={() => (auditRules.limite_combustivel_litros = 80)}
								class="px-1.5 py-0.5 rounded text-[10px] bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 transition-colors"
							>
								80 L (Carro)
							</button>
							<button
								type="button"
								on:click={() => (auditRules.limite_combustivel_litros = 250)}
								class="px-1.5 py-0.5 rounded text-[10px] bg-amber-500/20 hover:bg-amber-500/30 text-amber-300 border border-amber-500/40 transition-colors font-medium"
							>
								250 L (Padrão)
							</button>
							<button
								type="button"
								on:click={() => (auditRules.limite_combustivel_litros = 1000)}
								class="px-1.5 py-0.5 rounded text-[10px] bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 transition-colors"
							>
								1.000 L (Frota/Barco)
							</button>
						</div>
					</div>
				</div>

				<div class="p-5 bg-slate-900/60 border border-slate-800 rounded-xl space-y-4">
					<h3 class="text-sm font-semibold text-amber-300">Referência de combustível</h3>
					<p class="text-xs text-slate-400">Informe o preço e a fonte para o período e local analisados. A referência é manual; não há consulta automática à ANP.</p>
					<label class="block text-xs text-slate-300">Preço de referência (R$/L)
						<input type="number" min="0.01" max="100" step="0.01" bind:value={auditRules.preco_combustivel_referencia} class="mt-1 w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2" />
					</label>
					<label class="block text-xs text-slate-300">Margem de sobrepreço (%)
						<input type="number" min="0" max="1000" step="0.1" bind:value={auditRules.sobrepreco_combustivel_percentual} class="mt-1 w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2" />
					</label>
					<label class="flex gap-2 items-start text-xs text-slate-300">
						<input type="checkbox" bind:checked={auditRules.estimar_volume_combustivel} class="mt-0.5 accent-amber-500" />
						Estimar litros quando a nota não declarar volume (alerta de confiança reduzida)
					</label>
					<label class="block text-xs text-slate-300">Período da referência
						<input type="text" maxlength="120" placeholder="Ex.: setembro/2024" bind:value={auditRules.referencia_combustivel_periodo} class="mt-1 w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2" />
					</label>
					<label class="block text-xs text-slate-300">Local da referência
						<input type="text" maxlength="120" placeholder="Ex.: Campinas/SP" bind:value={auditRules.referencia_combustivel_local} class="mt-1 w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2" />
					</label>
					<label class="block text-xs text-slate-300">Link da fonte (opcional)
						<input type="url" maxlength="2048" placeholder="https://..." bind:value={auditRules.referencia_combustivel_url} class="mt-1 w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2" />
					</label>
				</div>

				<!-- Parâmetro 2: Janela de Triangulação -->
				<div class="p-5 bg-slate-900/60 border border-slate-800 rounded-xl flex flex-col justify-between space-y-4">
					<div class="space-y-2">
						<div class="flex items-center justify-between">
							<span class="text-xs font-semibold text-white flex items-center gap-1.5">
								<svg class="w-4 h-4 text-indigo-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
									<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
								</svg>
								Janela de Triangulação Societária
							</span>
							<span class="px-2 py-0.5 rounded text-xs font-mono font-bold bg-indigo-500/10 border border-indigo-500/30 text-indigo-300">
								{auditRules.janela_triangulacao_dias} dias
							</span>
						</div>
						<p class="text-[11px] text-slate-400 leading-snug">
							Janela de proximidade temporal entre abertura da empresa, alteração societária no QSA ou doação e a contratação pública/emissão de nota CEAP.
						</p>
					</div>

					<div class="space-y-3">
						<!-- Slider -->
						<div class="space-y-1">
							<input
								type="range"
								min="1"
								max="730"
								step="1"
								bind:value={auditRules.janela_triangulacao_dias}
								class="w-full accent-indigo-500 cursor-pointer h-2 bg-slate-800 rounded-lg"
							/>
							<div class="flex justify-between text-[10px] text-slate-500 font-mono">
								<span>1 dia</span>
								<span class="text-indigo-400/80">Padrão: 180 dias</span>
								<span>730 dias (2 anos)</span>
							</div>
						</div>

						<!-- Entrada Manual -->
						<div class="flex items-center gap-2">
							<label for="input-janela" class="text-[11px] text-slate-400 whitespace-nowrap">Entrada direta (1 a 730 dias):</label>
							<div class="relative flex-1">
								<input
									id="input-janela"
									type="number"
									min="1"
									max="730"
									step="1"
									bind:value={auditRules.janela_triangulacao_dias}
									class="w-full bg-slate-950 border border-slate-700 text-slate-100 text-xs rounded-lg px-2.5 py-1.5 pr-12 focus:ring-indigo-500 focus:border-indigo-500 font-mono"
								/>
								<span class="absolute right-2.5 top-1.5 text-xs text-slate-500 pointer-events-none">dias</span>
							</div>
						</div>

						<!-- Presets rápidos -->
						<div class="flex items-center gap-1.5 pt-1">
							<span class="text-[10px] text-slate-500">Atalhos:</span>
							<button
								type="button"
								on:click={() => (auditRules.janela_triangulacao_dias = 90)}
								class="px-1.5 py-0.5 rounded text-[10px] bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 transition-colors"
							>
								90 dias
							</button>
							<button
								type="button"
								on:click={() => (auditRules.janela_triangulacao_dias = 180)}
								class="px-1.5 py-0.5 rounded text-[10px] bg-indigo-500/20 hover:bg-indigo-500/30 text-indigo-300 border border-indigo-500/40 transition-colors font-medium"
							>
								180 dias (Recomendado)
							</button>
							<button
								type="button"
								on:click={() => (auditRules.janela_triangulacao_dias = 365)}
								class="px-1.5 py-0.5 rounded text-[10px] bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 transition-colors"
							>
								365 dias (1 ano)
							</button>
						</div>
					</div>
				</div>

				<!-- Parâmetro 3: Concentração de Fornecedor -->
				<div class="p-5 bg-slate-900/60 border border-slate-800 rounded-xl flex flex-col justify-between space-y-4">
					<div class="space-y-2">
						<div class="flex items-center justify-between">
							<span class="text-xs font-semibold text-white flex items-center gap-1.5">
								<svg class="w-4 h-4 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
									<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 3.055A9.001 9.001 0 1020.945 13H11V3.055z" />
									<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.488 9H15V3.512A9.025 9.025 0 0120.488 9z" />
								</svg>
								Concentração de Fornecedor Hub
							</span>
							<span class="px-2 py-0.5 rounded text-xs font-mono font-bold bg-cyan-500/10 border border-cyan-500/30 text-cyan-300">
								{auditRules.concentracao_fornecedor_percentual}%
							</span>
						</div>
						<p class="text-[11px] text-slate-400 leading-snug">
							Percentual de faturamento ou exclusividade da empresa com o mesmo parlamentar/gabinete que dispara suspeita de fornecedor dedicado/hub.
						</p>
					</div>

					<div class="space-y-3">
						<!-- Slider -->
						<div class="space-y-1">
							<input
								type="range"
								min="0"
								max="100"
								step="1"
								bind:value={auditRules.concentracao_fornecedor_percentual}
								class="w-full accent-cyan-500 cursor-pointer h-2 bg-slate-800 rounded-lg"
							/>
							<div class="flex justify-between text-[10px] text-slate-500 font-mono">
								<span>0%</span>
								<span class="text-cyan-400/80">Padrão: 60%</span>
								<span>100%</span>
							</div>
						</div>

						<!-- Entrada Manual -->
						<div class="flex items-center gap-2">
							<label for="input-concentracao" class="text-[11px] text-slate-400 whitespace-nowrap">Entrada direta (0% a 100%):</label>
							<div class="relative flex-1">
								<input
									id="input-concentracao"
									type="number"
									min="0"
									max="100"
									step="1"
									bind:value={auditRules.concentracao_fornecedor_percentual}
									class="w-full bg-slate-950 border border-slate-700 text-slate-100 text-xs rounded-lg px-2.5 py-1.5 pr-8 focus:ring-cyan-500 focus:border-cyan-500 font-mono"
								/>
								<span class="absolute right-2.5 top-1.5 text-xs text-slate-500 pointer-events-none">%</span>
							</div>
						</div>

						<!-- Presets rápidos -->
						<div class="flex items-center gap-1.5 pt-1">
							<span class="text-[10px] text-slate-500">Atalhos:</span>
							<button
								type="button"
								on:click={() => (auditRules.concentracao_fornecedor_percentual = 40)}
								class="px-1.5 py-0.5 rounded text-[10px] bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 transition-colors"
							>
								40% (Rigoroso)
							</button>
							<button
								type="button"
								on:click={() => (auditRules.concentracao_fornecedor_percentual = 60)}
								class="px-1.5 py-0.5 rounded text-[10px] bg-cyan-500/20 hover:bg-cyan-500/30 text-cyan-300 border border-cyan-500/40 transition-colors font-medium"
							>
								60% (Recomendado)
							</button>
							<button
								type="button"
								on:click={() => (auditRules.concentracao_fornecedor_percentual = 80)}
								class="px-1.5 py-0.5 rounded text-[10px] bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 transition-colors"
							>
								80% (Tolerante)
							</button>
						</div>
					</div>
				</div>
			</div>

			<!-- Botões de Ação e Mensagens de Feedback -->
			<div class="border-t border-slate-700/60 pt-4 flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-3">
				<div class="flex flex-wrap items-center gap-3">
					<button
						type="button"
						on:click={salvarAuditRules}
						disabled={salvandoAuditRules || carregandoAuditRules}
						class="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
					>
						{#if salvandoAuditRules}
							<svg class="w-3.5 h-3.5 animate-spin text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
							</svg>
							<span>Persistindo Parâmetros...</span>
						{:else}
							<svg class="w-4 h-4 text-emerald-200" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
							</svg>
							<span>Salvar Parâmetros</span>
						{/if}
					</button>

					<button
						type="button"
						on:click={restaurarPadroesAuditRules}
						disabled={salvandoAuditRules || carregandoAuditRules}
						class="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white text-xs font-medium rounded-lg border border-slate-700 transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
					>
						<svg class="w-3.5 h-3.5 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
						</svg>
						<span>Restaurar Padrões de Fábrica</span>
					</button>

					<button
						type="button"
						on:click={sincronizarMotorAuditoria}
						disabled={sincronizandoAuditoria}
						class="px-4 py-2 bg-indigo-600/30 hover:bg-indigo-600/50 text-indigo-300 hover:text-white text-xs font-semibold rounded-lg border border-indigo-500/40 transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
						title="Disparar varredura em lote com todas as heurísticas do Radar"
					>
						{#if sincronizandoAuditoria}
							<div class="w-3.5 h-3.5 border-2 border-indigo-400 border-t-transparent rounded-full animate-spin"></div>
							<span>Sincronizando Heurísticas...</span>
						{:else}
							<svg class="w-3.5 h-3.5 text-indigo-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
							</svg>
							<span>Varredura Completa de Auditoria</span>
						{/if}
					</button>
				</div>

				<span class="text-[11px] text-slate-500 text-right">
					Padrão: 250 L de combustível &bull; 180 dias de triangulação &bull; 60% de exclusividade
				</span>
			</div>

			{#if msgSincAuditoria}
				<div class="p-3 bg-indigo-500/10 border border-indigo-500/30 rounded-lg text-indigo-300 text-xs flex items-center gap-2 animate-fade-in">
					<span class="text-sm">⚡</span>
					<span>{msgSincAuditoria}</span>
				</div>
			{/if}

			{#if msgAuditRulesSucesso}
				<div class="p-3 bg-emerald-500/10 border border-emerald-500/30 rounded-lg text-emerald-300 text-xs flex items-center gap-2 animate-fade-in">
					<span class="text-sm">✅</span>
					<span>{msgAuditRulesSucesso}</span>
				</div>
			{/if}

			{#if erroAuditRules}
				<div class="p-3 bg-rose-500/10 border border-rose-500/30 rounded-lg text-rose-300 text-xs flex items-center gap-2 animate-fade-in">
					<span class="text-sm">❌</span>
					<span>{erroAuditRules}</span>
				</div>
			{/if}
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

		<!-- Card de Destaque: Sincronização Unificada (Sincronizar Tudo) -->
		<div class="mb-6 p-6 rounded-2xl bg-gradient-to-r from-slate-900 via-slate-850 to-emerald-950/40 border border-emerald-500/30 shadow-lg flex flex-col md:flex-row items-start md:items-center justify-between gap-4">
			<div class="space-y-1">
				<div class="flex items-center gap-2">
					<span class="px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">
						Pipeline Orquestrado
					</span>
					<span class="text-xs text-slate-400">TSE + CEAP + Sócios QSA + OAB + Auditoria</span>
				</div>
				<h3 class="text-lg font-bold text-white flex items-center gap-2">
					<span>Sincronização Unificada &bull; Sincronizar Tudo</span>
				</h3>
				<p class="text-xs text-slate-300 max-w-2xl leading-relaxed">
					Executa a varredura completa das bases oficiais de forma sequencial e idempotente. Políticos e candidaturas são consolidados via UPSERT e as notas da CEAP usam chave única anti-duplicidade, recalculando as anomalias do Motor de Auditoria ao final.
				</p>
			</div>

			<button
				type="button"
				on:click={() => (modalSincronizarTudo = true)}
				disabled={sincronizandoTudo || (activeJob?.fonte === 'SINCRONIZAR_TUDO' && activeJob?.status === 'PROCESSANDO')}
				class="flex-shrink-0 px-5 py-2.5 bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-white text-xs font-bold rounded-xl shadow-md shadow-emerald-950/50 transition-all disabled:opacity-50 flex items-center gap-2 cursor-pointer"
			>
				{#if sincronizandoTudo || (activeJob?.fonte === 'SINCRONIZAR_TUDO' && activeJob?.status === 'PROCESSANDO')}
					<svg class="w-4 h-4 animate-spin text-white" fill="none" viewBox="0 0 24 24">
						<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
						<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
					</svg>
					<span>Processando Pipeline...</span>
				{:else}
					<span class="text-base">🔄</span>
					<span>Sincronizar Tudo</span>
				{/if}
			</button>
		</div>

		<!-- Card de Destaque: Deduplicação Inteligente em Massa -->
		<div class="mb-6 p-6 rounded-2xl bg-gradient-to-r from-slate-900 via-indigo-950/40 to-slate-900 border border-indigo-500/30 shadow-lg flex flex-col md:flex-row items-start md:items-center justify-between gap-4">
			<div class="space-y-1">
				<div class="flex items-center gap-2">
					<span class="px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider bg-indigo-500/20 text-indigo-300 border border-indigo-500/30">
						Consolidação de Identidades
					</span>
					<span class="text-xs text-slate-400">Nome Civil + Data de Nascimento</span>
				</div>
				<h3 class="text-lg font-bold text-white flex items-center gap-2">
					<span>Deduplicação Inteligente em Massa &bull; Background Job</span>
				</h3>
				<p class="text-xs text-slate-300 max-w-2xl leading-relaxed">
					Analisa grupos com mesmo nome civil e data de nascimento (ex.: candidatos com candidaturas separadas em eleições distintas), unificando bens, mandatos e registros para um único perfil canônico sem perda de integridade.
				</p>
			</div>

			<button
				type="button"
				on:click={dispararDeduplicacaoMassa}
				disabled={deduplicandoMassa || (activeJob?.fonte === 'DEDUPLICACAO_MASSA' && activeJob?.status === 'PROCESSANDO')}
				class="flex-shrink-0 px-5 py-2.5 bg-gradient-to-r from-indigo-600 to-violet-600 hover:from-indigo-500 hover:to-violet-500 text-white text-xs font-bold rounded-xl shadow-md shadow-indigo-950/50 transition-all disabled:opacity-50 flex items-center gap-2 cursor-pointer"
			>
				{#if deduplicandoMassa || (activeJob?.fonte === 'DEDUPLICACAO_MASSA' && activeJob?.status === 'PROCESSANDO')}
					<svg class="w-4 h-4 animate-spin text-white" fill="none" viewBox="0 0 24 24">
						<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
						<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
					</svg>
					<span>Deduplicando em Segundo Plano...</span>
				{:else}
					<span class="text-base">🧬</span>
					<span>Executar Deduplicação em Massa</span>
				{/if}
			</button>
		</div>

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
					<div class="flex flex-wrap items-center gap-2">
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
							<option value={2016}>2016 (Municipal)</option>
							<option value={2014}>2014 (Geral)</option>
							<option value={2012}>2012 (Municipal)</option>
							<option value={2010}>2010 (Geral)</option>
							<option value={2008}>2008 (Municipal)</option>
							<option value={2006}>2006 (Geral)</option>
							<option value={2004}>2004 (Municipal)</option>
							<option value={2002}>2002 (Geral)</option>
							<option value={2000}>2000 (Municipal)</option>
							<option value={1998}>1998 (Geral)</option>
							<option value={1996}>1996 (Municipal)</option>
						</select>
						<div class="flex items-center gap-1.5 text-xs text-slate-400">
							<span class="text-[11px] text-slate-500">ou digite:</span>
							<input
								type="number"
								min="1990"
								max="2030"
								step="2"
								placeholder="Ano"
								bind:value={anoTse}
								class="w-20 bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-2 py-1.5 focus:ring-emerald-500 focus:border-emerald-500 text-center font-mono"
								title="Informe qualquer ano eleitoral do TSE"
							/>
						</div>
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

			<!-- Fonte 5: Autoridades de Cúpula (STF, TCU, PGR, Diplomacia, Secretarias) -->
			<div class="p-6 bg-slate-800/50 border border-slate-700/60 rounded-xl space-y-4 md:col-span-2">
				<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
					<div class="flex items-center gap-3">
						<div class="w-9 h-9 rounded-lg bg-indigo-500/10 border border-indigo-500/30 flex items-center justify-center text-indigo-400 font-bold text-sm">
							🏛️
						</div>
						<div>
							<h3 class="font-medium text-white">Autoridades Públicas de Cúpula (STF, TCU, PGR, MPTCU)</h3>
							<p class="text-xs text-slate-400">Ministros do STF e TCU, Procuradoria-Geral da República (PGR e MPTCU), Embaixadores e Secretários de Estado</p>
						</div>
					</div>

					<button
						type="button"
						on:click={dispararSincronizarAutoridades}
						disabled={sincronizandoAutoridades}
						class="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50 flex items-center gap-2 cursor-pointer self-start sm:self-auto"
					>
						{#if sincronizandoAutoridades}
							<svg class="w-3.5 h-3.5 animate-spin text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
							</svg>
							<span>Sincronizando...</span>
						{:else}
							<span>🏛️ Sincronizar Autoridades de Cúpula</span>
						{/if}
					</button>
				</div>
				<p class="text-xs text-slate-300 bg-slate-900/50 p-2.5 rounded-lg border border-slate-800/80 leading-relaxed">
					Ingere e sincroniza com deduplicação por nome civil e cruzamento eleitoral: os 11 Ministros do Supremo Tribunal Federal, os 9 Ministros titulares do Tribunal de Contas da União (TCU), Procuradores-Gerais da República e do MPTCU, embaixadores em postos diplomáticos estratégicos e secretários estaduais.
				</p>
			</div>

			<!-- Fonte 6: Diários Oficiais Municipais (Querido Diário) -->
			<div class="p-6 bg-slate-800/50 border border-slate-700/60 rounded-xl space-y-4 md:col-span-2">
				<div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
					<div class="flex items-center gap-3">
						<div class="w-9 h-9 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 font-bold text-sm">
							📰
						</div>
						<div>
							<h3 class="font-medium text-white">Diários Oficiais Municipais (Querido Diário)</h3>
							<p class="text-xs text-slate-400">Varredura e auditoria de atos de nomeação, portarias e contratos públicos em centenas de municípios</p>
						</div>
					</div>

					<button
						type="button"
						on:click={dispararSincronizarDiarios}
						disabled={sincronizandoDiarios}
						class="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold rounded-lg shadow-sm transition-colors disabled:opacity-50 flex items-center gap-2 cursor-pointer self-start sm:self-auto"
					>
						{#if sincronizandoDiarios}
							<svg class="w-3.5 h-3.5 animate-spin text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
							</svg>
							<span>Varrendo Diários...</span>
						{:else}
							<span>📰 Sincronizar Querido Diário</span>
						{/if}
					</button>
				</div>

				<div class="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-1">
					<div>
						<label for="termoDiarioInput" class="block text-[11px] font-medium text-slate-400 mb-1">Termo / Nome para Busca (Opcional - padrão: Top Doadores & Políticos)</label>
						<input
							id="termoDiarioInput"
							type="text"
							bind:value={termoDiario}
							placeholder="Ex: Nome de doador ou empresa fornecedora"
							class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-1.5 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-emerald-500"
						/>
					</div>
					<div>
						<label for="municipioDiarioInput" class="block text-[11px] font-medium text-slate-400 mb-1">Município (Opcional)</label>
						<input
							id="municipioDiarioInput"
							type="text"
							bind:value={municipioDiario}
							placeholder="Ex: São Paulo, Rio de Janeiro, Recife, Salvador..."
							class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-1.5 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-emerald-500"
						/>
					</div>
				</div>

				<p class="text-xs text-slate-300 bg-slate-900/50 p-2.5 rounded-lg border border-slate-800/80 leading-relaxed">
					Consulta a API pública do projeto Querido Diário (Open Knowledge Brasil) indexando gazetas municipais. Registra ocorrências de nomeação e cruzamentos com doadores e agentes públicos na tabela de auditoria local com persistência no SQLite.
				</p>
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
						 (activeJob.status === 'PARCIAL' || activeJob.status === 'INTERROMPIDO') ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30' :
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
								 (imp.stage === 'CANCELADO' || imp.stage === 'INTERROMPIDO') ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30' :
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
										 (imp.stage === 'CANCELADO' || imp.stage === 'INTERROMPIDO') ? 'bg-orange-500' : 'bg-indigo-500'}"
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
						<option value="EMENDAS">Emendas Parlamentares (SIOP / Portal da Transparência)</option>
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

							<div class="p-3 bg-slate-950/60 rounded border border-slate-800/80 space-y-1">
								<span class="font-semibold text-emerald-300 block">Emendas Parlamentares (SIOP / Transparência):</span>
								<p class="text-slate-400 text-[11px]">Colunas aceitas: <code class="text-slate-300">NUMERO_EMENDA</code>, <code class="text-slate-300">AUTOR_NOME</code>, <code class="text-slate-300">ANO</code>, <code class="text-slate-300">TIPO_EMENDA</code>, <code class="text-slate-300">VALOR_EMPENHADO</code>, <code class="text-slate-300">BENEFICIARIO_NOME</code>, <code class="text-slate-300">BENEFICIARIO_CNPJ</code></p>
								<p class="text-[10px] text-slate-500">Tabela de destino: <span class="font-mono text-indigo-300">emendas_parlamentares</span></p>
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
						<p class="text-xs text-slate-400">Cópia íntegra de produção (.sqlite) gerada em tempo real com WAL online</p>
					</div>
				</div>

				<div class="flex flex-wrap items-center gap-2 pt-2">
					<button
						on:click={baixarBanco}
						class="px-4 py-2 bg-amber-600 hover:bg-amber-500 text-slate-950 font-semibold text-xs rounded-lg transition-colors flex items-center justify-center gap-1.5 shadow-sm"
					>
						<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
						</svg>
						<span>Baixar Arquivo Agora</span>
					</button>

					<button
						on:click={criarBackupQuente}
						disabled={criandoBackup}
						class="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-amber-300 hover:text-white border border-amber-500/30 font-semibold text-xs rounded-lg transition-colors flex items-center justify-center gap-1.5 disabled:opacity-50"
					>
						{#if criandoBackup}
							<div class="w-3 h-3 border-2 border-amber-400 border-t-transparent rounded-full animate-spin"></div>
							<span>Salvando Snapshot...</span>
						{:else}
							<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
							</svg>
							<span>Criar Snapshot no Servidor</span>
						{/if}
					</button>
				</div>

				{#if msgBackup}
					<div class="p-2.5 rounded-lg bg-emerald-500/10 border border-emerald-500/30 text-emerald-300 text-xs">
						✅ {msgBackup}
					</div>
				{/if}
				{#if erroBackup}
					<div class="p-2.5 rounded-lg bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs">
						❌ {erroBackup}
					</div>
				{/if}

				<!-- Listagem de Backups Salvos -->
				{#if backups.length > 0}
					<div class="pt-2 border-t border-slate-700/60 space-y-2">
						<span class="text-[11px] font-semibold text-slate-400 uppercase tracking-wider block">
							Snapshots Salvos em Servidor ({backups.length}):
						</span>
						<div class="max-h-36 overflow-y-auto space-y-1.5 pr-1">
							{#each backups as b}
								<div class="p-2 bg-slate-900/80 border border-slate-700/50 rounded-lg flex items-center justify-between text-[11px]">
									<div>
										<span class="font-mono text-slate-200 block">{b.nome_arquivo}</span>
										<span class="text-slate-500 text-[10px]">{b.tamanho_formatado} &bull; {new Date(b.criado_em).toLocaleDateString('pt-BR')} {new Date(b.criado_em).toLocaleTimeString('pt-BR')}</span>
									</div>
									<a
										href={b.download_url}
										class="px-2.5 py-1 rounded bg-slate-800 hover:bg-slate-700 text-amber-300 text-[11px] font-medium transition-colors"
										download
									>
										Download
									</a>
								</div>
							{/each}
						</div>
					</div>
				{/if}
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

		<!-- Integração de Webhooks -->
		<div class="mt-6 p-6 bg-slate-800/40 border border-slate-700/60 rounded-xl space-y-4">
			<div class="flex items-center gap-3">
				<div class="w-10 h-10 rounded-lg bg-teal-500/10 border border-teal-500/30 flex items-center justify-center text-teal-400">
					<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
					</svg>
				</div>
				<div>
					<h3 class="font-medium text-white">Notificações Externas via Webhook</h3>
					<p class="text-xs text-slate-400">Envio de disparos HTTP POST para Slack, Discord ou endpoints de monitoramento</p>
				</div>
			</div>

			<div class="flex flex-col sm:flex-row items-stretch sm:items-center gap-2 pt-1">
				<input
					type="url"
					bind:value={webhookUrl}
					placeholder="https://exemplo.com/api/webhook ou https://discord.com/api/webhooks/..."
					class="flex-1 bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:ring-teal-500 focus:border-teal-500 font-mono"
				/>
				<button
					type="button"
					on:click={testarWebhook}
					disabled={testandoWebhook || !webhookUrl.trim()}
					class="px-4 py-2 bg-teal-600 hover:bg-teal-500 text-slate-950 font-semibold text-xs rounded-lg transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
				>
					{#if testandoWebhook}
						<div class="w-3.5 h-3.5 border-2 border-slate-950 border-t-transparent rounded-full animate-spin"></div>
						<span>Testando...</span>
					{:else}
						<span>Disparar Webhook de Teste</span>
					{/if}
				</button>
			</div>

			{#if resultadoWebhook}
				<div class="p-3 rounded-lg text-xs flex items-center gap-2 animate-fade-in {resultadoWebhook.sucesso ? 'bg-emerald-500/10 border border-emerald-500/30 text-emerald-300' : 'bg-rose-500/10 border border-rose-500/30 text-rose-300'}">
					<span>{resultadoWebhook.sucesso ? '✅' : '❌'}</span>
					<span>{resultadoWebhook.mensagem}</span>
					{#if resultadoWebhook.status_code}
						<span class="font-mono text-[11px] px-1.5 py-0.5 rounded bg-slate-900 border border-slate-700">HTTP {resultadoWebhook.status_code}</span>
					{/if}
				</div>
			{/if}
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

	<!-- Modal de Configuração & Confirmação: Sincronizar Tudo -->
	{#if modalSincronizarTudo}
		<div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm animate-fade-in">
			<div class="bg-slate-900 border border-slate-700/80 rounded-2xl max-w-lg w-full p-6 space-y-6 shadow-2xl text-slate-200">
				<!-- Cabeçalho do Modal -->
				<div class="flex items-center justify-between border-b border-slate-800 pb-4">
					<div class="flex items-center gap-3">
						<div class="w-10 h-10 rounded-xl bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 font-bold text-lg">
							🔄
						</div>
						<div>
							<h3 class="text-lg font-bold text-white">Sincronizar Tudo</h3>
							<p class="text-xs text-slate-400">Atualização Completa &bull; Radar Cívico</p>
						</div>
					</div>
					<button
						type="button"
						on:click={() => (modalSincronizarTudo = false)}
						class="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
					>
						<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
						</svg>
					</button>
				</div>

				<!-- Explicação de Idempotência e Tempo -->
				<div class="space-y-3 text-xs leading-relaxed text-slate-300">
					<div class="p-3.5 bg-emerald-950/30 border border-emerald-500/30 rounded-xl space-y-1.5">
						<div class="flex items-center gap-2 font-semibold text-emerald-300 text-xs">
							<span>🛡️ Idempotência e Proteção Anti-Duplicidade</span>
						</div>
						<p class="text-slate-300 text-[11px] leading-relaxed">
							O sistema não joga cópias repetidas no banco: políticos e candidaturas são atualizados via <strong>UPSERT</strong>, notas fiscais da CEAP usam <strong>índice único</strong> anti-duplicação, e o Motor de Auditoria recalcula apenas anomalias válidas.
						</p>
					</div>

					<div class="p-3 bg-slate-800/60 border border-slate-700/60 rounded-xl flex items-center justify-between text-xs">
						<span class="text-slate-400">⏱️ Tempo Médio Estimado:</span>
						<span class="font-semibold text-amber-300 font-mono">~8 a 15 minutos (em background)</span>
					</div>
				</div>

				<!-- Opções de Escopo e Anos -->
				<div class="space-y-3 pt-1">
					<p class="text-xs font-semibold text-white uppercase tracking-wider">Fontes a Sincronizar:</p>
					<div class="grid grid-cols-1 sm:grid-cols-2 gap-2 text-xs">
						<label class="flex items-center gap-2 p-2.5 bg-slate-800/40 border border-slate-700/60 rounded-lg cursor-pointer hover:bg-slate-800/70">
							<input type="checkbox" bind:checked={incluirCamaraTudo} class="rounded bg-slate-950 border-slate-600 text-emerald-500 focus:ring-emerald-500" />
							<span>Câmara CEAP ({anoCeap})</span>
						</label>

						<label class="flex items-center gap-2 p-2.5 bg-slate-800/40 border border-slate-700/60 rounded-lg cursor-pointer hover:bg-slate-800/70">
							<input type="checkbox" bind:checked={incluirTseTudo} class="rounded bg-slate-950 border-slate-600 text-emerald-500 focus:ring-emerald-500" />
							<span>TSE Eleitoral ({anoTse})</span>
						</label>

						<label class="flex items-center gap-2 p-2.5 bg-slate-800/40 border border-slate-700/60 rounded-lg cursor-pointer hover:bg-slate-800/70">
							<input type="checkbox" bind:checked={incluirReceitaTudo} class="rounded bg-slate-950 border-slate-600 text-emerald-500 focus:ring-emerald-500" />
							<span>Sócios QSA & OAB</span>
						</label>

						<label class="flex items-center gap-2 p-2.5 bg-slate-800/40 border border-slate-700/60 rounded-lg cursor-pointer hover:bg-slate-800/70">
							<input type="checkbox" bind:checked={incluirAutoridadesTudo} class="rounded bg-slate-950 border-slate-600 text-emerald-500 focus:ring-emerald-500" />
							<span>Autoridades de Cúpula (STF/PGR/MRE)</span>
						</label>

						<label class="flex items-center gap-2 p-2.5 bg-slate-800/40 border border-slate-700/60 rounded-lg cursor-pointer hover:bg-slate-800/70 sm:col-span-2">
							<input type="checkbox" bind:checked={incluirAuditoriaTudo} class="rounded bg-slate-950 border-slate-600 text-emerald-500 focus:ring-emerald-500" />
							<span>Motor de Auditoria (Recálculo de Heurísticas)</span>
						</label>
					</div>
				</div>

				<!-- Ações -->
				<div class="flex items-center justify-end gap-3 pt-3 border-t border-slate-800">
					<button
						type="button"
						on:click={() => (modalSincronizarTudo = false)}
						class="px-4 py-2 text-xs font-semibold text-slate-300 hover:text-white bg-slate-800 hover:bg-slate-700 rounded-lg border border-slate-700 transition-colors"
					>
						Cancelar
					</button>
					<button
						type="button"
						on:click={dispararSincronizarTudo}
						disabled={sincronizandoTudo || (!incluirCamaraTudo && !incluirTseTudo && !incluirReceitaTudo && !incluirAutoridadesTudo && !incluirAuditoriaTudo)}
						class="px-5 py-2 text-xs font-semibold text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg shadow-md transition-colors disabled:opacity-50 flex items-center gap-2 cursor-pointer"
					>
						{#if sincronizandoTudo}
							<svg class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
								<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
								<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
							</svg>
							<span>Iniciando...</span>
						{:else}
							<span>Iniciar Sincronização Agora</span>
						{/if}
					</button>
				</div>
			</div>
		</div>
	{/if}
</div>
