export interface ItemBusca {
	id: number;
	tipo: string;
	nome: string;
	identificador: string;
	detalhe: string | null;
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
				esfera?: string | null;
				uf?: string | null;
				municipio?: string | null;
			};
		}>;
		edges: Array<{
			data: {
				id: string;
				source: string;
				target: string;
				tipo_relacao: string;
				valor: number;
				ano: number;
				fonte_dado: string;
				anomalia: boolean;
			};
		}>;
	};
}
