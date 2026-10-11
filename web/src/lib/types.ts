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

export interface PontoEvolucaoPatrimonial {
	ano: number;
	cargo: string;
	valor_total: number;
	variacao_percentual_anterior: number | null;
	variacao_absoluta_anterior: number | null;
}

export interface AlertaEvolucaoPatrimonial {
	politico_id: number;
	politico_nome: string;
	ano_anterior: number;
	valor_anterior: number;
	ano_recente: number;
	valor_recente: number;
	variacao_percentual: number;
	incremento_absoluto: number;
	gravidade: string;
	motivo: string;
}

export interface CargoAutoridadeItem {
	id: number;
	orgao: string;
	cargo: string;
	esfera: string;
	uf: string | null;
	data_posse: string | null;
	data_exoneracao: string | null;
	ato_nomeacao: string | null;
	biografia_resumo: string | null;
	origem_dado: string | null;
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
	tipo_agente?: string | null;
	candidaturas: CandidaturaItem[];
	historico_bens: BemItem[];
	doadores: DoadorItem[];
	alertas_auxilio?: AlertaAuxilioItem[];
	evolucao_patrimonial?: PontoEvolucaoPatrimonial[];
	alertas_evolucao_patrimonial?: AlertaEvolucaoPatrimonial[];
	cargos_autoridades?: CargoAutoridadeItem[];
	score_integridade?: number;
	nivel_risco?: string;
	cor_risco_hex?: string;
	alertas_parentesco?: AlertaPossivelParentesco[];
	emendas?: EmendaItem[];
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
	volume_estimado?: boolean;
	volume_litros?: number | null;
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

export interface EmendaItem {
	id: number;
	ano: number;
	numero_emenda: string;
	tipo_emenda: string;
	localidade_destino: string;
	uf: string;
	beneficiario: string;
	valor_empenhado: number;
	valor_pago: number;
}

export interface PoliticoEmendasResponse {
	politico_id: number;
	autor_nome: string;
	total_empenhado: number;
	total_pago: number;
	total_emendas: number;
	emendas: EmendaItem[];
}

export interface AlertaPossivelParentesco {
	alvo_nome: string;
	alvo_documento: string;
	uf: string;
	tipo_vinculo: string;
	sobrenomes_compartilhados: string[];
	nivel_suspeicao: string;
	descricao: string;
}

export interface ItemPoliticoListagem {
	id: number;
	sq_candidato: string | null;
	cpf_mascarado: string | null;
	nome_completo: string;
	nome_urna: string;
	sigla_partido: string;
	uf: string;
	cargo: string;
	municipio: string | null;
	total_despesas_ceap: number;
	total_itens_ceap: number;
	total_bens_declarados: number;
	tem_alertas: boolean;
	foto_base64: string | null;
	foto_mime: string | null;
	foto_url?: string | null;
	mandatos?: string[];
	ano_eleicao?: number | null;
	tipo_agente?: string | null;
	score_integridade?: number;
	nivel_risco?: string;
	cor_risco_hex?: string;
}

export interface BuscarFotoResponse {
	sucesso: boolean;
	mensagem: string;
	foto_base64: string | null;
	foto_mime: string | null;
	origem: string | null;
}

export interface SalvarFotoManualRequest {
	foto_base64?: string;
	foto_url?: string;
	foto_mime?: string;
}

export interface ListarPoliticosResponse {
	total: number;
	page: number;
	limit: number;
	total_paginas: number;
	partidos_disponiveis: string[];
	ufs_disponiveis: string[];
	cargos_disponiveis: string[];
	anos_disponiveis?: number[];
	politicos: ItemPoliticoListagem[];
}

export interface DespesaCeapResumoItem {
	id: number;
	data_emissao: string;
	categoria_despesa: string;
	fornecedor_nome: string;
	fornecedor_cnpj_cpf: string;
	valor_liquido: number;
	detalhes_litros: number | null;
	numero_documento: string | null;
	url_nota_fiscal: string | null;
	flag_anomalia: boolean;
}

export interface GastoCategoriaItem {
	categoria: string;
	total: number;
	quantidade: number;
	percentual: number;
}

export interface ResumoFinanceiroPolitico {
	total_gasto_ceap: number;
	total_notas_ceap: number;
	media_mensal_ceap: number;
	total_bens_declarados: number;
	total_doacoes_campanha: number;
	total_fora_uf: number;
	notas_fora_uf: number;
	categoria_mais_gasta: string | null;
	valor_categoria_mais_gasta: number;
}

export interface PoliticoDetalheResponse {
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
	foto_url?: string | null;
	partido: string;
	uf: string;
	cargo: string;
	municipio: string | null;
	resumo_financeiro: ResumoFinanceiroPolitico;
	gastos_por_categoria: GastoCategoriaItem[];
	despesas_recentes: DespesaCeapResumoItem[];
	candidaturas: CandidaturaItem[];
	historico_bens: BemItem[];
	doadores: DoadorItem[];
	alertas_auxilio?: AlertaAuxilioItem[];
	evolucao_patrimonial?: PontoEvolucaoPatrimonial[];
	alertas_evolucao_patrimonial?: AlertaEvolucaoPatrimonial[];
	tipo_agente?: string | null;
	cargos_autoridades?: CargoAutoridadeItem[];
	score_integridade?: number;
	nivel_risco?: string;
	cor_risco_hex?: string;
	alertas_parentesco?: AlertaPossivelParentesco[];
	emendas?: EmendaItem[];
}

export interface BackupItemInfo {
	nome_arquivo: string;
	tamanho_bytes: number;
	tamanho_formatado: string;
	criado_em: string;
	download_url: string;
}

export interface PontoDespesaGeo {
	id: number;
	fornecedor_nome: string;
	fornecedor_cnpj: string;
	municipio: string;
	uf: string;
	latitude: number;
	longitude: number;
	valor: number;
	data: string;
	categoria: string;
	litros: number | null;
	numero_documento: string | null;
	url_documento: string | null;
	fora_uf_origem: boolean;
	alerta_distancia: boolean;
	distancia_origem_km: number;
	motivo_alerta: string | null;
}

export interface PoliticoDespesasGeoResponse {
	politico_id: number;
	politico_nome: string;
	politico_uf: string;
	total_despesas_geo: number;
	total_valor_geo: number;
	despesas_fora_uf_total: number;
	despesas_fora_uf_valor: number;
	pontos: PontoDespesaGeo[];
}

export interface ItemPoliticoDuplicado {
	id: number;
	sq_candidato: string | null;
	cpf_mascarado: string | null;
	nome_completo: string;
	nome_urna: string;
	data_nascimento: string | null;
	sigla_partido: string;
	uf: string;
	cargo: string;
	ano_eleicao?: number | null;
	mandatos: string[];
	total_despesas_ceap: number;
	total_itens_ceap: number;
	total_bens: number;
	foto_base64: string | null;
	foto_mime: string | null;
	foto_url?: string | null;
}

export interface GrupoPoliticosDuplicados {
	id_grupo: string;
	criterio: string;
	confianca: string;
	motivo: string;
	sugestao_canonico_id: number;
	politicos: ItemPoliticoDuplicado[];
}

export interface ResumoDuplicadosResponse {
	total_grupos: number;
	total_registros_duplicados: number;
	grupos_nascimento_exato: number;
	grupos_ceap_tse: number;
	explicacao_tecnica: string;
}

export interface RelatorioDuplicadosResponse {
	total_grupos: number;
	page: number;
	limit: number;
	total_paginas: number;
	resumo: ResumoDuplicadosResponse;
	grupos: GrupoPoliticosDuplicados[];
}

export interface MesclarPoliticosRequest {
	id_canonico: number;
	id_duplicado: number;
}

export interface MesclarPoliticosResponse {
	sucesso: boolean;
	mensagem: string;
	id_canonico: number;
	id_removido: number;
	candidaturas_migradas: number;
}


