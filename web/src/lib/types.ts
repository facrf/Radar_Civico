export interface ItemBusca {
	id: number | null;
	tipo: string;
	nome?: string;
	titulo?: string;
	identificador: string;
	detalhe?: string | null;
	subtitulo?: string | null;
	documento?: string | null;
}

export interface BemItem {
	id: number;
	candidatura_id: number;
	ano_eleicao: number;
	tipo_bem: string | null;
	descricao: string | null;
	valor_declarado: number;
}

export interface DoadorItem {
	id: number;
	candidatura_id: number;
	ano_eleicao: number;
	doador_cpf_cnpj: string;
	doador_nome: string;
	valor: number;
	data_receita: string | null;
	tipo_origem: string | null;
}

export interface CandidaturaItem {
	id: number;
	ano_eleicao: number;
	cargo: string;
	numero_urna: number | null;
	sigla_partido: string;
	uf: string;
	municipio: string | null;
	situacao_totalizacao: string | null;
	total_bens_declarados: number;
}

export interface AlertaAuxilioItem {
	id: number;
	motivo: string;
	detalhes: string | null;
	valor_recebido: number;
	total_bens: number | null;
	cargo_ou_mandato: string | null;
	ano_exercicio: number | null;
	status_analise: string;
	mes_disponibilizacao: string | null;
	parcela: string | null;
	data_alerta: string | null;
}

export interface DossiePolitico {
	id: number;
	sq_candidato: string | null;
	cpf_mascarado: string | null;
	nome_completo: string;
	nome_urna: string;
	data_nascimento: string | null;
	grau_instrucao: string | null;
	ocupacao: string | null;
	foto_base64: string | null;
	foto_mime: string | null;
	candidaturas: CandidaturaItem[];
	historico_bens: BemItem[];
	doadores: DoadorItem[];
	alertas_auxilio?: AlertaAuxilioItem[];
}

export interface AlertaItem {
	id: number;
	tipo: string;
	severidade: string;
	titulo: string;
	descricao: string;
	alvo_nome: string;
	alvo_documento: string | null;
	municipio: string | null;
	uf: string | null;
	ano: number | null;
	valor_envolvido: number | null;
	fonte_dado: string;
	detalhes: any;
	data_criacao: string | null;
}

export interface SubgrafoData {
	raiz_id: string;
	grau: number;
	cytoscape: {
		nodes: Array<{
			data: {
				id: string;
				label: string;
				tipo: string;
				documento?: string | null;
				subtitulo?: string | null;
				cor?: string;
			};
		}>;
		edges: Array<{
			data: {
				id: string;
				source: string;
				target: string;
				label: string;
				valor?: number;
				ano?: number;
			};
		}>;
	};
}

// ============================================================================
// Dossiê Analítico de Vínculos e Anomalias (CNPJ e CPF)
// ============================================================================

export interface AlertaDossie {
	tipo: string;
	severidade: 'INFO' | 'BAIXA' | 'MEDIA' | 'ALTA' | 'CRITICA';
	titulo: string;
	descricao: string;
	valor_envolvido: number | null;
	fonte: string;
}

export interface EmpresaResumo {
	cnpj: string;
	cnpj_formatado: string;
	razao_social: string;
	vinculo: string;
}

export interface SocioItem {
	nome: string;
	documento_mascarado: string;
	qualificacao: string | null;
	tipo: 'PF' | 'PJ';
	outras_empresas: EmpresaResumo[];
}

export interface PainelSocietario {
	total_socios: number;
	socios: SocioItem[];
	empresas_interligadas: EmpresaResumo[];
}

export interface NotaFiscalCeapItem {
	id: number;
	parlamentar_nome: string;
	data_emissao: string;
	categoria_despesa: string;
	valor_liquido: number;
	numero_documento: string | null;
	url_nota_fiscal: string | null;
	flag_anomalia: boolean;
	politico_id: number | null;
}

export interface CompradorCeapResumo {
	parlamentar_nome: string;
	total_gasto: number;
	quantidade_notas: number;
	politico_id: number | null;
}

export interface PainelCeap {
	total_faturado: number;
	total_notas: number;
	compradores: CompradorCeapResumo[];
	notas_fiscais: NotaFiscalCeapItem[];
}

export interface ContratoPncpItem {
	id: number;
	orgao_contratante: string;
	valor_contratado: number;
	objeto: string | null;
	data_assinatura: string | null;
	data_termino: string | null;
}

export interface OrgaoContratanteResumo {
	orgao: string;
	total_valor: number;
	quantidade_contratos: number;
}

export interface PainelPncp {
	total_contratado: number;
	total_contratos: number;
	orgaos_contratantes: OrgaoContratanteResumo[];
	contratos: ContratoPncpItem[];
}

export interface DoacaoSocioTse {
	socio_nome: string;
	doador_doc: string;
	candidato_nome: string;
	politico_id: number | null;
	cargo: string;
	partido: string;
	ano: number;
	valor: number;
	data: string | null;
}

export interface CandidaturaSocioTse {
	socio_nome: string;
	politico_id: number;
	nome_urna: string;
	cargo: string;
	partido: string;
	ano: number;
	uf: string;
	total_bens: number;
}

export interface PainelTseSocios {
	total_doacoes_socios: number;
	doacoes: DoacaoSocioTse[];
	candidaturas_socios: CandidaturaSocioTse[];
}

export interface DiarioItem {
	termo: string;
	data: string | null;
	ocorrencias: number;
	resumo: string | null;
}

export interface DossieCnpj {
	cnpj: string;
	cnpj_formatado: string;
	razao_social: string;
	situacao_cadastral: string;
	data_abertura: string | null;
	score_risco: 'BAIXO' | 'MEDIO' | 'ALTO' | 'CRITICO';
	total_alertas: number;
	alertas: AlertaDossie[];
	qsa: PainelSocietario;
	ceap: PainelCeap;
	pncp: PainelPncp;
	tse: PainelTseSocios;
	diarios: DiarioItem[];
}

export interface EmpresaSocioItem {
	cnpj: string;
	cnpj_formatado: string;
	razao_social: string;
	qualificacao: string | null;
	total_ceap: number;
	total_pncp: number;
}

export interface DoacaoEleitoralItem {
	id: number;
	doador_cpf_cnpj: string;
	doador_nome: string;
	valor: number;
	data_receita: string | null;
	ano_eleicao: number;
	cargo: string;
	partido: string;
	politico_nome: string;
	politico_id: number | null;
}

export interface CandidaturaItemCpf {
	politico_id: number;
	nome_urna: string;
	ano_eleicao: number;
	cargo: string;
	partido: string;
	uf: string;
	municipio: string | null;
	total_bens: number;
}

export interface BeneficioEmergencialItem {
	id: number;
	mes: string;
	parcela: string | null;
	valor: number;
	enquadramento: string | null;
}

export interface RegistroProfissionalItem {
	orgao: string;
	numero: string;
	uf: string;
	situacao: string;
	tipo: string | null;
}

export interface DossieCpf {
	cpf_mascarado: string;
	nome: string;
	score_risco: 'BAIXO' | 'MEDIO' | 'ALTO' | 'CRITICO';
	total_alertas: number;
	alertas: AlertaDossie[];
	empresas_socio: EmpresaSocioItem[];
	total_faturado_empresas_ceap: number;
	total_contratado_empresas_pncp: number;
	doacoes_eleitorais: DoacaoEleitoralItem[];
	candidaturas: CandidaturaItemCpf[];
	beneficios_emergenciais: BeneficioEmergencialItem[];
	registros_profissionais: RegistroProfissionalItem[];
	notas_ceap_empresas: NotaFiscalCeapItem[];
	contratos_pncp_empresas: ContratoPncpItem[];
}
