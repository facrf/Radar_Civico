export interface TotalRegistros {
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

export interface ConfigStatus {
	tamanho_banco_bytes: number;
	tamanho_banco_formatado: string;
	caminho_banco: string;
	total_registros: TotalRegistros;
	ultimo_evento_sincronizacao: string | null;
	versao_sistema: string;
	git_commit?: string;
	commit_count?: number;
}

export interface IdentidadeVisual {
	tem_icone_customizado: boolean;
	tem_favicon_customizado: boolean;
	icone_url: string;
	favicon_url: string;
}

export interface JobInfo {
	job_id: string;
	fonte: string;
	ano?: number;
	status: 'PENDENTE' | 'PROCESSANDO' | 'CONCLUIDO' | 'PARCIAL' | 'INTERROMPIDO' | 'ERRO';
	persistencia_pendente?: boolean;
	progresso: number;
	mensagem: string;
	logs: string[];
	criado_em: string;
	concluido_em?: string;
}

export interface UploadResult {
	status: string;
	arquivo: string;
	tipo_detectado: string;
	registros_inseridos: number;
        avisos?: string[];
	mensagem: string;
}

export interface ParametrosAuditoria {
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

export interface ImportProgress {
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

export interface ImporterSummary {
	id: string;
	name: string;
	description: string;
	stage: 'IDLE' | 'CONECTANDO' | 'BAIXANDO' | 'DESCOMPACTANDO' | 'PROCESSANDO' | 'FINALIZANDO' | 'CONCLUIDO' | 'PARCIAL' | 'CANCELADO' | 'INTERROMPIDO' | 'ERRO';
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

export interface ToastNotificacao {
	id: number;
	tipo: 'erro' | 'sucesso' | 'info';
	mensagem: string;
}

export interface BackupItem {
	nome_arquivo: string;
	tamanho_bytes: number;
	tamanho_formatado: string;
	criado_em: string;
	download_url: string;
}
