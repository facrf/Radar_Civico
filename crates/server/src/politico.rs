use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use storage::rusqlite::Connection;
use storage::DbPool;

use crate::geo::resolver_coordenadas_despesa;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BemItem {
    pub id: i64,
    pub candidatura_id: i64,
    pub ano_eleicao: i32,
    pub tipo_bem: Option<String>,
    pub descricao: Option<String>,
    pub valor_declarado: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DoadorItem {
    pub id: i64,
    pub candidatura_id: i64,
    pub ano_eleicao: i32,
    pub doador_cpf_cnpj: String,
    pub doador_nome: String,
    pub valor: f64,
    pub data_receita: Option<String>,
    pub tipo_origem: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidaturaItem {
    pub id: i64,
    pub ano_eleicao: i32,
    pub cargo: String,
    pub numero_urna: Option<i32>,
    pub sigla_partido: String,
    pub uf: String,
    pub municipio: Option<String>,
    pub situacao_totalizacao: Option<String>,
    pub total_bens_declarados: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaAuxilioItem {
    pub id: i64,
    pub motivo: String,
    pub detalhes: Option<String>,
    pub valor_recebido: f64,
    pub total_bens: Option<f64>,
    pub cargo_ou_mandato: Option<String>,
    pub ano_exercicio: Option<i32>,
    pub status_analise: String,
    pub mes_disponibilizacao: Option<String>,
    pub parcela: Option<String>,
    pub data_alerta: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DossiePolitico {
    pub id: i64,
    pub sq_candidato: Option<String>,
    pub cpf_mascarado: Option<String>,
    pub nome_completo: String,
    pub nome_urna: String,
    pub data_nascimento: Option<String>,
    pub grau_instrucao: Option<String>,
    pub ocupacao: Option<String>,
    pub foto_base64: Option<String>,
    pub foto_mime: Option<String>,
    pub foto_url: Option<String>,
    pub candidaturas: Vec<CandidaturaItem>,
    pub historico_bens: Vec<BemItem>,
    pub doadores: Vec<DoadorItem>,
    #[serde(default)]
    pub alertas_auxilio: Vec<AlertaAuxilioItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DespesaCeapResumoItem {
    pub id: i64,
    pub data_emissao: String,
    pub categoria_despesa: String,
    pub fornecedor_nome: String,
    pub fornecedor_cnpj_cpf: String,
    pub valor_liquido: f64,
    pub detalhes_litros: Option<f64>,
    pub numero_documento: Option<String>,
    pub url_nota_fiscal: Option<String>,
    pub flag_anomalia: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GastoCategoriaItem {
    pub categoria: String,
    pub total: f64,
    pub quantidade: i64,
    pub percentual: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResumoFinanceiroPolitico {
    pub total_gasto_ceap: f64,
    pub total_notas_ceap: i64,
    pub media_mensal_ceap: f64,
    pub total_bens_declarados: f64,
    pub total_doacoes_campanha: f64,
    pub total_fora_uf: f64,
    pub notas_fora_uf: i64,
    pub categoria_mais_gasta: Option<String>,
    pub valor_categoria_mais_gasta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoliticoDetalheResponse {
    pub id: i64,
    pub sq_candidato: Option<String>,
    pub cpf_mascarado: Option<String>,
    pub nome_completo: String,
    pub nome_urna: String,
    pub data_nascimento: Option<String>,
    pub grau_instrucao: Option<String>,
    pub ocupacao: Option<String>,
    pub foto_base64: Option<String>,
    pub foto_mime: Option<String>,
    pub foto_url: Option<String>,
    pub partido: String,
    pub uf: String,
    pub cargo: String,
    pub municipio: Option<String>,
    pub resumo_financeiro: ResumoFinanceiroPolitico,
    pub gastos_por_categoria: Vec<GastoCategoriaItem>,
    pub despesas_recentes: Vec<DespesaCeapResumoItem>,
    pub candidaturas: Vec<CandidaturaItem>,
    pub historico_bens: Vec<BemItem>,
    pub doadores: Vec<DoadorItem>,
    pub alertas_auxilio: Vec<AlertaAuxilioItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PontoDespesaGeo {
    pub id: i64,
    pub fornecedor_nome: String,
    pub fornecedor_cnpj: String,
    pub municipio: String,
    pub uf: String,
    pub latitude: f64,
    pub longitude: f64,
    pub valor: f64,
    pub data: String,
    pub categoria: String,
    pub litros: Option<f64>,
    pub numero_documento: Option<String>,
    pub url_documento: Option<String>,
    pub fora_uf_origem: bool,
    pub alerta_distancia: bool,
    pub distancia_origem_km: f64,
    pub motivo_alerta: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoliticoDespesasGeoResponse {
    pub politico_id: i64,
    pub politico_nome: String,
    pub politico_uf: String,
    pub total_despesas_geo: usize,
    pub total_valor_geo: f64,
    pub despesas_fora_uf_total: usize,
    pub despesas_fora_uf_valor: f64,
    pub pontos: Vec<PontoDespesaGeo>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListarPoliticosQueryParams {
    pub q: Option<String>,
    pub partido: Option<String>,
    pub uf: Option<String>,
    pub cargo: Option<String>,
    pub ano: Option<i32>,
    pub ano_eleicao: Option<i32>,
    pub apenas_com_gastos: Option<bool>,
    pub page: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemPoliticoListagem {
    pub id: i64,
    pub sq_candidato: Option<String>,
    pub cpf_mascarado: Option<String>,
    pub nome_completo: String,
    pub nome_urna: String,
    pub sigla_partido: String,
    pub uf: String,
    pub cargo: String,
    pub municipio: Option<String>,
    pub total_despesas_ceap: f64,
    pub total_itens_ceap: i64,
    pub total_bens_declarados: f64,
    pub tem_alertas: bool,
    pub foto_base64: Option<String>,
    pub foto_mime: Option<String>,
    pub foto_url: Option<String>,
    #[serde(default)]
    pub mandatos: Vec<String>,
    #[serde(default)]
    pub ano_eleicao: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuscarFotoResponse {
    pub sucesso: bool,
    pub mensagem: String,
    pub foto_base64: Option<String>,
    pub foto_mime: Option<String>,
    pub origem: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SalvarFotoManualRequest {
    pub foto_base64: Option<String>,
    pub foto_url: Option<String>,
    pub foto_mime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListarPoliticosResponse {
    pub total: usize,
    pub page: usize,
    pub limit: usize,
    pub total_paginas: usize,
    pub partidos_disponiveis: Vec<String>,
    pub ufs_disponiveis: Vec<String>,
    pub cargos_disponiveis: Vec<String>,
    #[serde(default)]
    pub anos_disponiveis: Vec<i32>,
    pub politicos: Vec<ItemPoliticoListagem>,
}

/// Mapeamento auxiliar de deputados federais conhecidos e bancadas para inferir Partido e UF.
pub fn inferir_partido_e_uf_deputado(nome: &str) -> (String, String) {
    let n = nome.to_uppercase();

    // Lideranças e bancadas
    if n.contains("LID.GOV") || n.contains("LIDERANÇA DO GOVERNO") {
        return ("GOV".to_string(), "DF".to_string());
    }
    if n.contains("PT") {
        return ("PT".to_string(), "DF".to_string());
    }
    if n.contains("PSDB") {
        return ("PSDB".to_string(), "DF".to_string());
    }
    if n.contains("PDT") {
        return ("PDT".to_string(), "DF".to_string());
    }
    if n.contains("PSOL") {
        return ("PSOL".to_string(), "DF".to_string());
    }
    if n.contains("REPUBLICANOS") {
        return ("REPUBLICANOS".to_string(), "DF".to_string());
    }
    if n.contains("PSD") {
        return ("PSD".to_string(), "DF".to_string());
    }
    if n.contains("PP") {
        return ("PP".to_string(), "DF".to_string());
    }
    if n.contains("PL") {
        return ("PL".to_string(), "DF".to_string());
    }
    if n.contains("UNIÃO") || n.contains("UNIAO") {
        return ("UNIÃO".to_string(), "DF".to_string());
    }

    // Deputados notáveis da 57ª legislatura
    let mapa_deputados = [
        ("DENISE PESSÔA", "PT", "RS"),
        ("DENISE PESSOA", "PT", "RS"),
        ("ALEXANDRE LINDENMEYER", "PT", "RS"),
        ("MARIA DO ROSÁRIO", "PT", "RS"),
        ("MARIA DO ROSARIO", "PT", "RS"),
        ("BOHN GASS", "PT", "RS"),
        ("MARCON", "PT", "RS"),
        ("AFONSO HAMM", "PP", "RS"),
        ("AFONSO MOTTA", "PDT", "RS"),
        ("ALCEU MOREIRA", "MDB", "RS"),
        ("ANY ORTIZ", "CIDADANIA", "RS"),
        ("DIMAS FABIANO", "PP", "MG"),
        ("PATRUS ANANIAS", "PT", "MG"),
        ("ANDRÉ JANONES", "AVANTE", "MG"),
        ("ANA PIMENTEL", "PT", "MG"),
        ("ANA PAULA LEÃO", "PP", "MG"),
        ("JORGE SOLLA", "PT", "BA"),
        ("ADOLFO VIANA", "PSDB", "BA"),
        ("AFONSO FLORENCE", "PT", "BA"),
        ("ALEX SANTANA", "REPUBLICANOS", "BA"),
        ("ALICE PORTUGAL", "PCdoB", "BA"),
        ("ANTONIO BRITO", "PSD", "BA"),
        ("DIEGO GARCIA", "REPUBLICANOS", "PR"),
        ("ALIEL MACHADO", "PV", "PR"),
        ("NATÁLIA BONAVIDES", "PT", "RN"),
        ("NATALIA BONAVIDES", "PT", "RN"),
        ("ABILIO BRUNINI", "PL", "MT"),
        ("AMÁLIA BARROS", "PL", "MT"),
        ("ACÁCIO FAVACHO", "MDB", "AP"),
        ("ACACIO FAVACHO", "MDB", "AP"),
        ("ADAIL FILHO", "REPUBLICANOS", "AM"),
        ("AMOM MANDEL", "CIDADANIA", "AM"),
        ("ADRIANA VENTURA", "NOVO", "SP"),
        ("ALBERTO MOURÃO", "MDB", "SP"),
        ("ALENCAR SANTANA", "PT", "SP"),
        ("ALEX MANENTE", "CIDADANIA", "SP"),
        ("ALEXANDRE LEITE", "UNIÃO", "SP"),
        ("ALFREDINHO", "PT", "SP"),
        ("ANTONIO CARLOS RODRIGUES", "PL", "SP"),
        ("ARLINDO CHINAGLIA", "PT", "SP"),
        ("ADRIANO DO BALDY", "PP", "GO"),
        ("AGUINALDO RIBEIRO", "PP", "PB"),
        ("AIRTON FALEIRO", "PT", "PA"),
        ("ANDREIA SIQUEIRA", "MDB", "PA"),
        ("ANTÔNIO DOIDO", "MDB", "PA"),
        ("ALBERTO FRAGA", "PL", "DF"),
        ("ALEXANDRE GUIMARÃES", "MDB", "TO"),
        ("ANTONIO ANDRADE", "REPUBLICANOS", "TO"),
        ("ALFREDO GASPAR", "UNIÃO", "AL"),
        ("ALLAN GARCÊS", "PP", "MA"),
        ("ALUISIO MENDES", "REPUBLICANOS", "MA"),
        ("AMANDA GENTIL", "PP", "MA"),
        ("ALTINEU CÔRTES", "PL", "RJ"),
        ("ALTINEU CORTES", "PL", "RJ"),
        ("AMARO NETO", "REPUBLICANOS", "ES"),
        ("ANA PAULA LIMA", "PT", "SC"),
        ("ANDRÉ FERNANDES", "PL", "CE"),
        ("ANDRE FERNANDES", "PL", "CE"),
        ("ANDRÉ FIGUEIREDO", "PDT", "CE"),
        ("ANDRÉ FERREIRA", "PL", "PE"),
        ("ANDRÉ FUFUCA", "PP", "MA"),
        ("ANTÔNIA LÚCIA", "REPUBLICANOS", "AC"),
    ];

    for (cand_nome, part, uf) in mapa_deputados {
        if n.contains(cand_nome) || cand_nome.contains(&n) {
            return (part.to_string(), uf.to_string());
        }
    }

    ("PL".to_string(), "DF".to_string())
}

/// Sincroniza parlamentares distintos de despesas_parlamentares na tabela politicos e candidaturas
pub fn sincronizar_parlamentares_ceap(conn: &Connection) -> Result<usize, storage::StorageError> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT parlamentar_nome, parlamentar_cpf_mascarado
         FROM despesas_parlamentares
         WHERE parlamentar_nome IS NOT NULL AND TRIM(parlamentar_nome) != ''",
    )?;

    let parlamentares = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    let mut novos = 0;
    for (nome, cpf) in parlamentares {
        let existe: bool = conn
            .query_row(
                "SELECT 1 FROM politicos WHERE UPPER(nome_completo) = UPPER(?1) OR UPPER(nome_urna) = UPPER(?1) LIMIT 1",
                [&nome],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !existe {
            let (partido, uf) = inferir_partido_e_uf_deputado(&nome);
            let ocupacao = if nome.starts_with("LID") || nome.starts_with("LIDERANÇA") {
                "LIDERANÇA PARTIDÁRIA"
            } else {
                "DEPUTADO FEDERAL"
            };

            conn.execute(
                "INSERT INTO politicos (nome_completo, nome_urna, cpf_mascarado, ocupacao)
                 VALUES (?1, ?1, ?2, ?3)",
                storage::rusqlite::params![nome, cpf, ocupacao],
            )?;
            let novo_id = conn.last_insert_rowid();

            conn.execute(
                "INSERT OR IGNORE INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf, municipio)
                 VALUES (?1, 2022, 'DEPUTADO FEDERAL', ?2, ?3, 'Brasília')",
                storage::rusqlite::params![novo_id, partido, uf],
            )?;
            novos += 1;
        }
    }

    Ok(novos)
}

pub fn carregar_dossie(pool: &DbPool, politico_id: i64) -> Result<Option<DossiePolitico>, storage::StorageError> {
    let conn = pool.get()?;

    let mut stmt = conn.prepare(
        "SELECT id, sq_candidato, cpf_mascarado, nome_completo, nome_urna,
                data_nascimento, grau_instrucao, ocupacao, foto_blob, foto_mime, foto_url
         FROM politicos WHERE id = ?1",
    )?;

    let politico_opt = stmt
        .query_row([politico_id], |row| {
            let foto_blob: Option<Vec<u8>> = row.get(8)?;
            let foto_base64 = foto_blob.map(|b| BASE64.encode(b));

            Ok(DossiePolitico {
                id: row.get(0)?,
                sq_candidato: row.get(1)?,
                cpf_mascarado: row.get(2)?,
                nome_completo: row.get(3)?,
                nome_urna: row.get(4)?,
                data_nascimento: row.get(5)?,
                grau_instrucao: row.get(6)?,
                ocupacao: row.get(7)?,
                foto_base64,
                foto_mime: row.get(9)?,
                foto_url: row.get(10)?,
                candidaturas: Vec::new(),
                historico_bens: Vec::new(),
                doadores: Vec::new(),
                alertas_auxilio: Vec::new(),
            })
        })
        .ok();

    let mut dossie = match politico_opt {
        Some(d) => d,
        None => return Ok(None),
    };

    // Candidaturas
    {
        let mut stmt_cand = conn.prepare(
            "SELECT id, ano_eleicao, cargo, numero_urna, sigla_partido, uf,
                    municipio, situacao_totalizacao, total_bens_declarados
             FROM candidaturas
             WHERE politico_id = ?1
             ORDER BY ano_eleicao DESC",
        )?;

        let rows = stmt_cand.query_map([politico_id], |row| {
            Ok(CandidaturaItem {
                id: row.get(0)?,
                ano_eleicao: row.get(1)?,
                cargo: row.get(2)?,
                numero_urna: row.get(3)?,
                sigla_partido: row.get(4)?,
                uf: row.get(5)?,
                municipio: row.get(6)?,
                situacao_totalizacao: row.get(7)?,
                total_bens_declarados: row.get(8)?,
            })
        })?;

        for r in rows.flatten() {
            dossie.candidaturas.push(r);
        }
    }

    // Histórico de Bens
    {
        let mut stmt_bens = conn.prepare(
            "SELECT b.id, b.candidatura_id, c.ano_eleicao, b.tipo_bem, b.descricao, b.valor_declarado
             FROM bens_candidato b
             JOIN candidaturas c ON b.candidatura_id = c.id
             WHERE c.politico_id = ?1
             ORDER BY c.ano_eleicao DESC, b.valor_declarado DESC",
        )?;

        let rows = stmt_bens.query_map([politico_id], |row| {
            Ok(BemItem {
                id: row.get(0)?,
                candidatura_id: row.get(1)?,
                ano_eleicao: row.get(2)?,
                tipo_bem: row.get(3)?,
                descricao: row.get(4)?,
                valor_declarado: row.get(5)?,
            })
        })?;

        for r in rows.flatten() {
            dossie.historico_bens.push(r);
        }
    }

    // Doadores
    {
        let mut stmt_doadores = conn.prepare(
            "SELECT r.id, r.candidatura_id, c.ano_eleicao, r.doador_cpf_cnpj, r.doador_nome,
                    r.valor, r.data_receita, r.tipo_origem
             FROM receitas_campanha r
             JOIN candidaturas c ON r.candidatura_id = c.id
             WHERE c.politico_id = ?1
             ORDER BY r.valor DESC",
        )?;

        let rows = stmt_doadores.query_map([politico_id], |row| {
            Ok(DoadorItem {
                id: row.get(0)?,
                candidatura_id: row.get(1)?,
                ano_eleicao: row.get(2)?,
                doador_cpf_cnpj: row.get(3)?,
                doador_nome: row.get(4)?,
                valor: row.get(5)?,
                data_receita: row.get(6)?,
                tipo_origem: row.get(7)?,
            })
        })?;

        for r in rows.flatten() {
            dossie.doadores.push(r);
        }
    }

    // Alertas de auxílio
    {
        let mut stmt_aux = conn.prepare(
            "SELECT a.id, a.motivo, a.detalhes, a.valor_recebido, a.total_bens,
                    a.cargo_ou_mandato, a.ano_exercicio, a.status_analise,
                    b.mes_disponibilizacao, b.parcela, a.data_alerta
             FROM alertas_beneficio_indevido a
             LEFT JOIN beneficios_emergenciais b ON a.beneficio_id = b.id
             WHERE a.politico_id = ?1
             ORDER BY a.valor_recebido DESC, a.id DESC",
        )?;

        let rows = stmt_aux.query_map([politico_id], |row| {
            Ok(AlertaAuxilioItem {
                id: row.get(0)?,
                motivo: row.get(1)?,
                detalhes: row.get(2)?,
                valor_recebido: row.get(3)?,
                total_bens: row.get(4)?,
                cargo_ou_mandato: row.get(5)?,
                ano_exercicio: row.get(6)?,
                status_analise: row.get(7)?,
                mes_disponibilizacao: row.get(8)?,
                parcela: row.get(9)?,
                data_alerta: row.get(10)?,
            })
        })?;

        for r in rows.flatten() {
            dossie.alertas_auxilio.push(r);
        }
    }

    Ok(Some(dossie))
}

/// Handler legado para dossiê básico de político
pub async fn politico_dossie_handler(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<Json<DossiePolitico>, StatusCode> {
    match carregar_dossie(&pool, id) {
        Ok(Some(dossie)) => Ok(Json(dossie)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// GET /api/politicos e /api/v1/politicos
pub async fn listar_politicos_handler(
    State(pool): State<DbPool>,
    Query(params): Query<ListarPoliticosQueryParams>,
) -> Result<Json<ListarPoliticosResponse>, (StatusCode, String)> {
    let conn = pool.get().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    let q_term = params.q.as_deref().unwrap_or("").trim();
    let filtro_partido = params.partido.as_deref().unwrap_or("").trim();
    let filtro_uf = params.uf.as_deref().unwrap_or("").trim();
    let filtro_cargo = params.cargo.as_deref().unwrap_or("").trim();
    let apenas_gastos = params.apenas_com_gastos.unwrap_or(false);

    // Listas distintas para preenchimento dos filtros no frontend
    let mut partidos_disponiveis = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT DISTINCT sigla_partido FROM candidaturas WHERE sigla_partido IS NOT NULL AND sigla_partido != '' ORDER BY sigla_partido ASC") {
        if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
            for p in rows.flatten() {
                partidos_disponiveis.push(p);
            }
        }
    }

    let mut ufs_disponiveis = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT DISTINCT uf FROM candidaturas WHERE uf IS NOT NULL AND uf != '' AND length(uf) = 2 ORDER BY uf ASC") {
        if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
            for u in rows.flatten() {
                ufs_disponiveis.push(u);
            }
        }
    }

    let mut anos_disponiveis = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT DISTINCT ano_eleicao FROM candidaturas WHERE ano_eleicao IS NOT NULL AND ano_eleicao > 1900 ORDER BY ano_eleicao DESC") {
        if let Ok(rows) = stmt.query_map([], |r| r.get::<_, i32>(0)) {
            for a in rows.flatten() {
                anos_disponiveis.push(a);
            }
        }
    }
    if anos_disponiveis.is_empty() {
        anos_disponiveis = vec![2024, 2022, 2020, 2018];
    }

    let cargos_disponiveis = vec![
        "DEPUTADO FEDERAL".to_string(),
        "VEREADOR".to_string(),
        "PREFEITO".to_string(),
        "VICE-PREFEITO".to_string(),
        "SENADOR".to_string(),
    ];

    // Construção dinâmica da query
    let mut where_clauses = Vec::new();
    let mut sql_params: Vec<Box<dyn storage::rusqlite::ToSql>> = Vec::new();

    // 1. Busca textual inteligente (nome completo, nome de urna ou CPF real diferente de -4)
    if !q_term.is_empty() {
        let q_clean = q_term.replace('.', "").replace('-', "");
        if q_clean.len() >= 3 && q_clean.chars().all(|c| c.is_ascii_digit()) {
            where_clauses.push(
                "(UPPER(p.nome_completo) LIKE ? OR UPPER(p.nome_urna) LIKE ? OR (p.cpf_mascarado IS NOT NULL AND p.cpf_mascarado != '-4' AND p.cpf_mascarado LIKE ?))"
                    .to_string(),
            );
            let q_like = format!("%{}%", q_term.to_uppercase());
            sql_params.push(Box::new(q_like.clone()));
            sql_params.push(Box::new(q_like));
            sql_params.push(Box::new(format!("%{}%", q_clean)));
        } else {
            where_clauses.push(
                "(UPPER(p.nome_completo) LIKE ? OR UPPER(p.nome_urna) LIKE ?)"
                    .to_string(),
            );
            let q_like = format!("%{}%", q_term.to_uppercase());
            sql_params.push(Box::new(q_like.clone()));
            sql_params.push(Box::new(q_like));
        }
    }

    // 2. Filtro por Partido
    if !filtro_partido.is_empty() {
        where_clauses.push("UPPER(c.sigla_partido) = ?".to_string());
        sql_params.push(Box::new(filtro_partido.to_uppercase()));
    }

    // 3. Filtro por UF
    if !filtro_uf.is_empty() {
        where_clauses.push("UPPER(c.uf) = ?".to_string());
        sql_params.push(Box::new(filtro_uf.to_uppercase()));
    }

    // 4. Filtro por Cargo com distinção estrita (evita que PREFEITO traga VICE-PREFEITO e vice-versa)
    if !filtro_cargo.is_empty() {
        let cargo_upper = filtro_cargo.to_uppercase();
        if cargo_upper == "VEREADOR" {
            where_clauses.push("(UPPER(c.cargo) LIKE '%VEREADOR%' OR c.cargo = '13')".to_string());
        } else if cargo_upper == "PREFEITO" {
            where_clauses.push("((UPPER(c.cargo) LIKE '%PREFEITO%' AND UPPER(c.cargo) NOT LIKE '%VICE%') OR c.cargo = '11')".to_string());
        } else if cargo_upper == "VICE-PREFEITO" {
            where_clauses.push("(UPPER(c.cargo) LIKE '%VICE-PREFEITO%' OR c.cargo = '12')".to_string());
        } else if cargo_upper == "DEPUTADO FEDERAL" {
            where_clauses.push("(UPPER(c.cargo) LIKE '%DEPUTADO FEDERAL%' OR c.cargo = '6' OR UPPER(c.cargo) = 'DEPUTADO')".to_string());
        } else if cargo_upper == "SENADOR" {
            where_clauses.push("(UPPER(c.cargo) LIKE '%SENADOR%' OR c.cargo = '5')".to_string());
        } else {
            where_clauses.push("(UPPER(c.cargo) LIKE ?)".to_string());
            let clike = format!("%{}%", cargo_upper);
            sql_params.push(Box::new(clike));
        }
    }

    // 5. Filtro por Ano da Eleição
    let filtro_ano = params.ano.or(params.ano_eleicao);
    if let Some(ano) = filtro_ano {
        if ano > 1900 {
            where_clauses.push("c.ano_eleicao = ?".to_string());
            sql_params.push(Box::new(ano));
        }
    }

    // 6. Filtro Apenas com Gastos CEAP (usa subquery rápida indexada evitando produto cartesiano)
    if apenas_gastos {
        where_clauses.push(
            "(p.nome_urna IN (SELECT DISTINCT parlamentar_nome FROM despesas_parlamentares WHERE parlamentar_nome IS NOT NULL) \
             OR p.nome_completo IN (SELECT DISTINCT parlamentar_nome FROM despesas_parlamentares WHERE parlamentar_nome IS NOT NULL))"
                .to_string(),
        );
    }

    let where_str = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    let params_refs: Vec<&dyn storage::rusqlite::ToSql> =
        sql_params.iter().map(|b| b.as_ref()).collect();

    // Contagem rápida e precisa
    let sql_contagem = format!(
        "SELECT COUNT(DISTINCT p.id)
         FROM politicos p
         JOIN candidaturas c ON c.politico_id = p.id
         {}",
        where_str
    );

    let total: usize = conn
        .query_row(&sql_contagem, params_refs.as_slice(), |r| r.get(0))
        .unwrap_or(0);

    // Ordenação: se usuário está navegando sem busca específica, prioriza deputados federais e mais recentes
    let order_clause = if !q_term.is_empty() {
        "p.nome_urna ASC"
    } else if !filtro_cargo.is_empty() {
        "c.ano_eleicao DESC, p.nome_urna ASC"
    } else {
        "(CASE WHEN c.cargo = 'DEPUTADO FEDERAL' THEN 1 WHEN c.cargo LIKE '%PREFEITO%' THEN 2 ELSE 3 END) ASC, c.ano_eleicao DESC, p.id ASC"
    };

    let sql_dados = format!(
        "SELECT p.id, p.sq_candidato, p.cpf_mascarado, p.nome_completo, p.nome_urna,
                COALESCE(c.sigla_partido, 'S/P'), COALESCE(c.uf, 'BR'),
                COALESCE(c.cargo, 'PARLAMENTAR'), c.municipio,
                COALESCE(c.total_bens_declarados, 0.0),
                p.foto_blob, p.foto_mime, p.foto_url,
                GROUP_CONCAT(DISTINCT c.cargo || CASE WHEN c.ano_eleicao IS NOT NULL AND c.ano_eleicao > 0 THEN ' (' || c.ano_eleicao || ')' ELSE '' END) as mandatos_str,
                MAX(c.ano_eleicao) as ano_eleicao
         FROM politicos p
         JOIN candidaturas c ON c.politico_id = p.id
         {}
         GROUP BY p.id
         ORDER BY {}
         LIMIT {} OFFSET {}",
        where_str, order_clause, limit, offset
    );

    let mut stmt_dados = conn
        .prepare(&sql_dados)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut politicos: Vec<ItemPoliticoListagem> = stmt_dados
        .query_map(params_refs.as_slice(), |row| {
            let id: i64 = row.get(0)?;
            let foto_blob: Option<Vec<u8>> = row.get(10)?;
            let foto_base64 = foto_blob.map(|b| BASE64.encode(b));
            let cargo: String = row.get(7)?;
            let mandatos_str: Option<String> = row.get(13)?;
            let mandatos = mandatos_str
                .map(|s| {
                    s.split(',')
                        .map(|m| m.trim().to_string())
                        .filter(|m| !m.is_empty())
                        .collect()
                })
                .unwrap_or_else(|| vec![cargo.clone()]);
            let ano_eleicao: Option<i32> = row.get(14).ok();

            Ok(ItemPoliticoListagem {
                id,
                sq_candidato: row.get(1)?,
                cpf_mascarado: row.get(2)?,
                nome_completo: row.get(3)?,
                nome_urna: row.get(4)?,
                sigla_partido: row.get(5)?,
                uf: row.get(6)?,
                cargo,
                municipio: row.get(8)?,
                total_despesas_ceap: 0.0,
                total_itens_ceap: 0,
                total_bens_declarados: row.get(9)?,
                tem_alertas: false,
                foto_base64,
                foto_mime: row.get(11)?,
                foto_url: row.get(12)?,
                mandatos,
                ano_eleicao,
            })
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    // Preenche despesas CEAP de forma indexada e instantânea para os itens paginados
    for pol in &mut politicos {
        let ceap_res: Option<(f64, i64)> = conn
            .query_row(
                "SELECT COALESCE(SUM(valor_liquido), 0.0), COUNT(id)
                  FROM despesas_parlamentares
                  WHERE parlamentar_nome = ?1 OR parlamentar_nome = ?2",
                [&pol.nome_urna, &pol.nome_completo],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();

        if let Some((tot, qtd)) = ceap_res {
            pol.total_despesas_ceap = tot;
            pol.total_itens_ceap = qtd;
            pol.tem_alertas = tot > 350000.0;
        }
    }

    if apenas_gastos {
        politicos.sort_by(|a, b| b.total_despesas_ceap.partial_cmp(&a.total_despesas_ceap).unwrap_or(std::cmp::Ordering::Equal));
    }

    let total_paginas = if total == 0 {
        1
    } else {
        (total + limit - 1) / limit
    };

    Ok(Json(ListarPoliticosResponse {
        total,
        page,
        limit,
        total_paginas,
        partidos_disponiveis,
        ufs_disponiveis,
        cargos_disponiveis,
        anos_disponiveis,
        politicos,
    }))
}

/// GET /api/politicos/:id e /api/v1/politicos/:id
pub async fn politico_detalhe_handler(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<Json<PoliticoDetalheResponse>, StatusCode> {
    let conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, sq_candidato, cpf_mascarado, nome_completo, nome_urna,
                    data_nascimento, grau_instrucao, ocupacao, foto_blob, foto_mime, foto_url
             FROM politicos WHERE id = ?1",
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (id_pol, sq, cpf_masc, nome_completo, nome_urna, dt_nasc, grau, ocup, foto_blob, foto_mime, foto_url) =
        match stmt.query_row([id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<Vec<u8>>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
            ))
        }) {
            Ok(tuple) => tuple,
            Err(_) => return Err(StatusCode::NOT_FOUND),
        };

    let foto_base64 = foto_blob.map(|b| BASE64.encode(b));

    // Carrega dados eleitorais básicos da candidatura mais recente
    let (partido, uf, cargo, mun_cand, total_bens_cand) = conn
        .query_row(
            "SELECT sigla_partido, uf, cargo, municipio, total_bens_declarados
             FROM candidaturas WHERE politico_id = ?1
             ORDER BY ano_eleicao DESC LIMIT 1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, f64>(4)?,
                ))
            },
        )
        .unwrap_or_else(|_| {
            let (part, est) = inferir_partido_e_uf_deputado(&nome_completo);
            (part, est, ocup.clone().unwrap_or_else(|| "DEPUTADO FEDERAL".to_string()), None, 0.0)
        });

    // 1. Resumo financeiro e histórico de despesas CEAP
    let mut stmt_ceap = conn
        .prepare(
            "SELECT id, data_emissao, categoria_despesa, fornecedor_nome, fornecedor_cnpj_cpf,
                    valor_liquido, detalhes_litros, numero_documento, url_nota_fiscal, flag_anomalia
             FROM despesas_parlamentares
             WHERE parlamentar_nome = ?1 OR parlamentar_nome = ?2
             ORDER BY data_emissao DESC",
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut despesas_recentes = Vec::new();
    let mut total_gasto_ceap = 0.0;
    let mut total_notas_ceap = 0;
    let mut total_fora_uf = 0.0;
    let mut notas_fora_uf = 0;
    let mut map_categorias: std::collections::HashMap<String, (f64, i64)> = std::collections::HashMap::new();

    let rows_ceap = stmt_ceap
        .query_map([&nome_completo, &nome_urna], |r| {
            Ok(DespesaCeapResumoItem {
                id: r.get(0)?,
                data_emissao: r.get(1)?,
                categoria_despesa: r.get(2)?,
                fornecedor_nome: r.get(3)?,
                fornecedor_cnpj_cpf: r.get(4)?,
                valor_liquido: r.get(5)?,
                detalhes_litros: r.get(6)?,
                numero_documento: r.get(7)?,
                url_nota_fiscal: r.get(8)?,
                flag_anomalia: r.get(9)?,
            })
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for r in rows_ceap.flatten() {
        total_gasto_ceap += r.valor_liquido;
        total_notas_ceap += 1;

        let entry = map_categorias
            .entry(r.categoria_despesa.clone())
            .or_insert((0.0, 0));
        entry.0 += r.valor_liquido;
        entry.1 += 1;

        let geo = resolver_coordenadas_despesa(
            &conn,
            &r.fornecedor_nome,
            &r.fornecedor_cnpj_cpf,
            &r.categoria_despesa,
            r.valor_liquido,
            &uf,
        );

        if geo.fora_uf_origem {
            total_fora_uf += r.valor_liquido;
            notas_fora_uf += 1;
        }

        if despesas_recentes.len() < 25 {
            despesas_recentes.push(r);
        }
    }

    let mut gastos_por_categoria = Vec::new();
    let mut cat_mais_gasta = None;
    let mut val_cat_mais_gasta = 0.0;

    for (cat, (val, qtd)) in map_categorias {
        let pct = if total_gasto_ceap > 0.0 {
            ((val / total_gasto_ceap) * 1000.0).round() / 10.0
        } else {
            0.0
        };

        if val > val_cat_mais_gasta {
            val_cat_mais_gasta = val;
            cat_mais_gasta = Some(cat.clone());
        }

        gastos_por_categoria.push(GastoCategoriaItem {
            categoria: cat,
            total: (val * 100.0).round() / 100.0,
            quantidade: qtd,
            percentual: pct,
        });
    }

    gastos_por_categoria.sort_by(|a, b| b.total.partial_cmp(&a.total).unwrap_or(std::cmp::Ordering::Equal));

    let media_mensal_ceap = if total_gasto_ceap > 0.0 {
        ((total_gasto_ceap / 12.0) * 100.0).round() / 100.0
    } else {
        0.0
    };

    // Carrega doações recebidas de campanha
    let total_doacoes_campanha: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(r.valor), 0.0)
             FROM receitas_campanha r
             JOIN candidaturas c ON r.candidatura_id = c.id
             WHERE c.politico_id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let resumo_financeiro = ResumoFinanceiroPolitico {
        total_gasto_ceap: (total_gasto_ceap * 100.0).round() / 100.0,
        total_notas_ceap,
        media_mensal_ceap,
        total_bens_declarados: total_bens_cand,
        total_doacoes_campanha: (total_doacoes_campanha * 100.0).round() / 100.0,
        total_fora_uf: (total_fora_uf * 100.0).round() / 100.0,
        notas_fora_uf,
        categoria_mais_gasta: cat_mais_gasta,
        valor_categoria_mais_gasta: (val_cat_mais_gasta * 100.0).round() / 100.0,
    };

    // Carrega dossiê detalhado (candidaturas, bens, doadores, auxílios)
    let dossie_base = carregar_dossie(&pool, id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or(DossiePolitico {
            id: id_pol,
            sq_candidato: sq.clone(),
            cpf_mascarado: cpf_masc.clone(),
            nome_completo: nome_completo.clone(),
            nome_urna: nome_urna.clone(),
            data_nascimento: dt_nasc.clone(),
            grau_instrucao: grau.clone(),
            ocupacao: ocup.clone(),
            foto_base64: foto_base64.clone(),
            foto_mime: foto_mime.clone(),
            foto_url: foto_url.clone(),
            candidaturas: Vec::new(),
            historico_bens: Vec::new(),
            doadores: Vec::new(),
            alertas_auxilio: Vec::new(),
        });

    Ok(Json(PoliticoDetalheResponse {
        id: id_pol,
        sq_candidato: sq,
        cpf_mascarado: cpf_masc,
        nome_completo,
        nome_urna,
        data_nascimento: dt_nasc,
        grau_instrucao: grau,
        ocupacao: ocup,
        foto_base64,
        foto_mime,
        foto_url,
        partido,
        uf,
        cargo,
        municipio: mun_cand,
        resumo_financeiro,
        gastos_por_categoria,
        despesas_recentes,
        candidaturas: dossie_base.candidaturas,
        historico_bens: dossie_base.historico_bens,
        doadores: dossie_base.doadores,
        alertas_auxilio: dossie_base.alertas_auxilio,
    }))
}

/// GET /api/politicos/:id/despesas-geo e /api/v1/politicos/:id/despesas-geo
pub async fn politico_despesas_geo_handler(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<Json<PoliticoDespesasGeoResponse>, StatusCode> {
    let conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (nome_completo, nome_urna) = match conn.query_row(
        "SELECT nome_completo, nome_urna FROM politicos WHERE id = ?1",
        [id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    ) {
        Ok(res) => res,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };

    let uf_politico: String = conn
        .query_row(
            "SELECT uf FROM candidaturas WHERE politico_id = ?1 ORDER BY ano_eleicao DESC LIMIT 1",
            [id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| {
            let (_, est) = inferir_partido_e_uf_deputado(&nome_completo);
            est
        });

    let mut stmt = conn
        .prepare(
            "SELECT id, data_emissao, categoria_despesa, fornecedor_nome, fornecedor_cnpj_cpf,
                    valor_liquido, detalhes_litros, numero_documento, url_nota_fiscal
             FROM despesas_parlamentares
             WHERE parlamentar_nome = ?1 OR parlamentar_nome = ?2
             ORDER BY data_emissao DESC
             LIMIT 500",
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut pontos = Vec::new();
    let mut total_valor_geo = 0.0;
    let mut despesas_fora_uf_total = 0;
    let mut despesas_fora_uf_valor = 0.0;

    let rows = stmt
        .query_map([&nome_completo, &nome_urna], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, Option<f64>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
            ))
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for r in rows.flatten() {
        let (id_desp, dt, cat, forn_nome, forn_cnpj, val, litros, num_doc, url_doc) = r;
        total_valor_geo += val;

        let geo = resolver_coordenadas_despesa(&conn, &forn_nome, &forn_cnpj, &cat, val, &uf_politico);

        if geo.fora_uf_origem {
            despesas_fora_uf_total += 1;
            despesas_fora_uf_valor += val;
        }

        pontos.push(PontoDespesaGeo {
            id: id_desp,
            fornecedor_nome: forn_nome,
            fornecedor_cnpj: forn_cnpj,
            municipio: geo.municipio,
            uf: geo.uf,
            latitude: geo.latitude,
            longitude: geo.longitude,
            valor: (val * 100.0).round() / 100.0,
            data: dt,
            categoria: cat,
            litros,
            numero_documento: num_doc,
            url_documento: url_doc,
            fora_uf_origem: geo.fora_uf_origem,
            alerta_distancia: geo.alerta_distancia,
            distancia_origem_km: geo.distancia_origem_km,
            motivo_alerta: geo.motivo_alerta,
        });
    }

    Ok(Json(PoliticoDespesasGeoResponse {
        politico_id: id,
        politico_nome: nome_completo,
        politico_uf: uf_politico,
        total_despesas_geo: pontos.len(),
        total_valor_geo: (total_valor_geo * 100.0).round() / 100.0,
        despesas_fora_uf_total,
        despesas_fora_uf_valor: (despesas_fora_uf_valor * 100.0).round() / 100.0,
        pontos,
    }))
}

/// POST /api/politicos/:id/buscar-foto-tse e /api/v1/politicos/:id/buscar-foto-tse
pub async fn buscar_foto_tse_handler(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<Json<BuscarFotoResponse>, (StatusCode, Json<BuscarFotoResponse>)> {
    let conn = pool.get().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BuscarFotoResponse {
                sucesso: false,
                mensagem: format!("Erro ao obter conexão do banco de dados: {}", e),
                foto_base64: None,
                foto_mime: None,
                origem: None,
            }),
        )
    })?;

    // 1. Verifica se já possui foto salva no banco de dados local
    let foto_existente: Option<(Vec<u8>, Option<String>)> = conn
        .query_row(
            "SELECT foto_blob, foto_mime FROM politicos WHERE id = ?1",
            [id],
            |r| {
                let blob: Option<Vec<u8>> = r.get(0)?;
                let mime: Option<String> = r.get(1)?;
                Ok((blob, mime))
            },
        )
        .ok()
        .and_then(|(b, m)| b.map(|bytes| (bytes, m)));

    if let Some((bytes, mime)) = foto_existente {
        if !bytes.is_empty() {
            let mime_str = mime.unwrap_or_else(|| "image/jpeg".to_string());
            let b64 = BASE64.encode(&bytes);
            return Ok(Json(BuscarFotoResponse {
                sucesso: true,
                mensagem: "Foto oficial já armazenada no banco de dados local.".to_string(),
                foto_base64: Some(b64),
                foto_mime: Some(mime_str),
                origem: Some("BANCO_LOCAL".to_string()),
            }));
        }
    }

    // 2. Consulta dados do político e suas candidaturas
    let dados_politico: Option<(Option<String>, String, String, Option<String>, String, i32)> = conn
        .query_row(
            "SELECT p.sq_candidato, p.nome_completo, p.nome_urna, p.ocupacao,
                    COALESCE(c.cargo, p.ocupacao, 'PARLAMENTAR'), COALESCE(c.ano_eleicao, 2024)
             FROM politicos p
             LEFT JOIN candidaturas c ON c.politico_id = p.id
             WHERE p.id = ?1
             ORDER BY c.ano_eleicao DESC LIMIT 1",
            [id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .ok();

    let (sq_opt, nome_completo, nome_urna, _ocup, cargo, ano_eleicao) = match dados_politico {
        Some(d) => d,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(BuscarFotoResponse {
                    sucesso: false,
                    mensagem: "Político não encontrado no banco de dados.".to_string(),
                    foto_base64: None,
                    foto_mime: None,
                    origem: None,
                }),
            ));
        }
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_default();

    let mut foto_baixada: Option<(Vec<u8>, String, String)> = None;
    let mut erros_detalhados: Vec<String> = Vec::new();

    // 2.5 Se tiver foto_url já cadastrada, tenta baixar diretamente dela primeiro
    let foto_url_cadastrada: Option<String> = conn
        .query_row("SELECT foto_url FROM politicos WHERE id = ?1", [id], |r| r.get(0))
        .ok()
        .flatten();

    if let Some(ref url_direta) = foto_url_cadastrada {
        if let Ok(resp_img) = client.get(url_direta).send().await {
            if resp_img.status().is_success() {
                if let Ok(bytes) = resp_img.bytes().await {
                    if !bytes.is_empty() {
                        foto_baixada = Some((bytes.to_vec(), "image/jpeg".to_string(), "CAMARA_DEPUTADOS".to_string()));
                    }
                }
            }
        }
    }

    // 3. Se for Deputado Federal (ou Câmara), consulta API da Câmara dos Deputados
    if foto_baixada.is_none() && (cargo.to_uppercase().contains("DEPUTADO") || cargo.to_uppercase().contains("PARLAMENTAR") || sq_opt.is_none()) {
        let nomes_para_buscar = vec![nome_urna.clone(), nome_completo.clone()];
        for n in nomes_para_buscar {
            if foto_baixada.is_some() {
                break;
            }
            let url_camara_api = format!(
                "https://dadosabertos.camara.leg.br/api/v2/deputados?nome={}&ordem=ASC&ordenarPor=nome",
                urlencoding::encode(&n)
            );

            match client.get(&url_camara_api)
                .header("User-Agent", "RadarCivico/0.1.0")
                .header("Accept", "application/json")
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(dados) = json.get("dados").and_then(|d| d.as_array()) {
                            if let Some(primeiro) = dados.first() {
                                if let Some(url_foto) = primeiro.get("urlFoto").and_then(|u| u.as_str()) {
                                    if let Ok(resp_img) = client.get(url_foto).send().await {
                                        if resp_img.status().is_success() {
                                            if let Ok(bytes) = resp_img.bytes().await {
                                                if !bytes.is_empty() {
                                                    foto_baixada = Some((bytes.to_vec(), "image/jpeg".to_string(), "CAMARA_DEPUTADOS".to_string()));
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(resp) => {
                    erros_detalhados.push(format!("Câmara API retornou status {}", resp.status()));
                }
                Err(e) => {
                    erros_detalhados.push(format!("Falha na conexão com API da Câmara: {}", e));
                }
            }
        }
    }

    // 4. Se ainda não baixou e tem sq_candidato, tenta TSE DivulgaCandContas
    if foto_baixada.is_none() {
        if let Some(sq) = sq_opt.filter(|s| !s.trim().is_empty()) {
            let urls_tse = vec![
                format!("https://divulgacandcontas.tse.jus.br/divulga/rest/v1/candidatura/buscar/foto/{}/{}", ano_eleicao, sq),
                format!("https://divulgacandcontas.tse.jus.br/divulga/rest/v1/candidatura/buscar/foto/{}", sq),
            ];

            for url in urls_tse {
                match client.get(&url)
                    .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                    .header("Referer", "https://divulgacandcontas.tse.jus.br/divulga/")
                    .header("Accept", "image/*,*/*")
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => {
                        let content_type = resp.headers().get("content-type")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("image/jpeg")
                            .to_string();

                        if let Ok(bytes) = resp.bytes().await {
                            if bytes.len() > 100 && (bytes.starts_with(&[0xFF, 0xD8]) || bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47])) {
                                foto_baixada = Some((bytes.to_vec(), content_type, "TSE_DIVULGACAND".to_string()));
                                break;
                            }
                        }
                    }
                    Ok(resp) => {
                        erros_detalhados.push(format!("TSE retornou HTTP {}", resp.status()));
                    }
                    Err(e) => {
                        erros_detalhados.push(format!("Erro ao acessar TSE: {}", e));
                    }
                }
            }
        } else {
            erros_detalhados.push("Candidato não possui código SQ_CANDIDATO cadastrado no banco.".to_string());
        }
    }

    // 5. Se obteve a foto, persiste no banco SQLite
    if let Some((bytes, mime, origem)) = foto_baixada {
        let _ = conn.execute(
            "UPDATE politicos SET foto_blob = ?1, foto_mime = ?2 WHERE id = ?3",
            storage::rusqlite::params![bytes, mime, id],
        );

        let b64 = BASE64.encode(&bytes);
        return Ok(Json(BuscarFotoResponse {
            sucesso: true,
            mensagem: format!("Foto oficial obtida via {} e salva com sucesso no banco de dados!", origem),
            foto_base64: Some(b64),
            foto_mime: Some(mime),
            origem: Some(origem),
        }));
    }

    // 6. Caso não tenha encontrado ou tenha falhado, retorna mensagem explicativa de erro
    let mensagem_erro = if erros_detalhados.is_empty() {
        "Não foi possível localizar a foto oficial no TSE ou Câmara para este parlamentar.".to_string()
    } else {
        format!(
            "Não foi possível obter a foto oficial: {}. Você pode utilizar a opção de upload manual ou fornecer uma URL direta de imagem.",
            erros_detalhados.join("; ")
        )
    };

    Err((
        StatusCode::NOT_FOUND,
        Json(BuscarFotoResponse {
            sucesso: false,
            mensagem: mensagem_erro,
            foto_base64: None,
            foto_mime: None,
            origem: None,
        }),
    ))
}

/// POST /api/politicos/:id/foto e /api/v1/politicos/:id/foto
pub async fn salvar_foto_manual_handler(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    Json(payload): Json<SalvarFotoManualRequest>,
) -> Result<Json<BuscarFotoResponse>, (StatusCode, Json<BuscarFotoResponse>)> {
    let conn = pool.get().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BuscarFotoResponse {
                sucesso: false,
                mensagem: format!("Erro ao obter conexão do banco de dados: {}", e),
                foto_base64: None,
                foto_mime: None,
                origem: None,
            }),
        )
    })?;

    let mut bytes_finais = Vec::new();
    let mut mime_final = payload.foto_mime.unwrap_or_else(|| "image/jpeg".to_string());

    if let Some(b64) = payload.foto_base64 {
        let clean_b64 = if let Some(pos) = b64.find("base64,") {
            &b64[pos + 7..]
        } else {
            &b64
        };
        match BASE64.decode(clean_b64.trim()) {
            Ok(b) if !b.is_empty() => {
                bytes_finais = b;
            }
            _ => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(BuscarFotoResponse {
                        sucesso: false,
                        mensagem: "Formato Base64 inválido fornecido.".to_string(),
                        foto_base64: None,
                        foto_mime: None,
                        origem: None,
                    }),
                ));
            }
        }
    } else if let Some(url) = payload.foto_url {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap_or_default();

        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Some(ct) = resp.headers().get("content-type").and_then(|c| c.to_str().ok()) {
                    mime_final = ct.to_string();
                }
                if let Ok(b) = resp.bytes().await {
                    bytes_finais = b.to_vec();
                }
            }
            Ok(resp) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(BuscarFotoResponse {
                        sucesso: false,
                        mensagem: format!("URL retornou status HTTP {}.", resp.status()),
                        foto_base64: None,
                        foto_mime: None,
                        origem: None,
                    }),
                ));
            }
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(BuscarFotoResponse {
                        sucesso: false,
                        mensagem: format!("Erro ao baixar imagem da URL: {}", e),
                        foto_base64: None,
                        foto_mime: None,
                        origem: None,
                    }),
                ));
            }
        }
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BuscarFotoResponse {
                sucesso: false,
                mensagem: "Informe 'foto_base64' ou 'foto_url' para salvar a imagem.".to_string(),
                foto_base64: None,
                foto_mime: None,
                origem: None,
            }),
        ));
    }

    if bytes_finais.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(BuscarFotoResponse {
                sucesso: false,
                mensagem: "Nenhum dado de imagem válido foi recebido.".to_string(),
                foto_base64: None,
                foto_mime: None,
                origem: None,
            }),
        ));
    }

    let res = conn.execute(
        "UPDATE politicos SET foto_blob = ?1, foto_mime = ?2 WHERE id = ?3",
        storage::rusqlite::params![bytes_finais, mime_final, id],
    );

    match res {
        Ok(rows) if rows > 0 => {
            let b64_resp = BASE64.encode(&bytes_finais);
            Ok(Json(BuscarFotoResponse {
                sucesso: true,
                mensagem: "Foto oficial salva com sucesso no banco de dados!".to_string(),
                foto_base64: Some(b64_resp),
                foto_mime: Some(mime_final),
                origem: Some("UPLOAD_MANUAL".to_string()),
            }))
        }
        _ => Err((
            StatusCode::NOT_FOUND,
            Json(BuscarFotoResponse {
                sucesso: false,
                mensagem: "Político não encontrado no banco de dados.".to_string(),
                foto_base64: None,
                foto_mime: None,
                origem: None,
            }),
        )),
    }
}

/// GET /api/politicos/:id/foto e /api/v1/politicos/:id/foto
pub async fn obter_foto_handler(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<axum::response::Response, StatusCode> {
    let conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let res: Option<(Vec<u8>, Option<String>)> = conn
        .query_row(
            "SELECT foto_blob, foto_mime FROM politicos WHERE id = ?1",
            [id],
            |r| {
                let blob: Option<Vec<u8>> = r.get(0)?;
                let mime: Option<String> = r.get(1)?;
                Ok((blob, mime))
            },
        )
        .ok()
        .and_then(|(b, m)| b.map(|bytes| (bytes, m)));

    match res {
        Some((bytes, mime)) if !bytes.is_empty() => {
            let mime_str = mime.unwrap_or_else(|| "image/jpeg".to_string());
            axum::response::Response::builder()
                .header(axum::http::header::CONTENT_TYPE, mime_str)
                .header(axum::http::header::CACHE_CONTROL, "public, max-age=86400")
                .body(axum::body::Body::from(bytes))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
        _ => {
            use axum::response::IntoResponse;
            let url_opt: Option<String> = conn
                .query_row("SELECT foto_url FROM politicos WHERE id = ?1", [id], |r| r.get(0))
                .ok()
                .flatten();

            if let Some(url) = url_opt {
                Ok(axum::response::Redirect::temporary(&url).into_response())
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
    }
}

/// DELETE /api/politicos/:id/foto e /api/v1/politicos/:id/foto
pub async fn remover_foto_handler(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
) -> Result<Json<BuscarFotoResponse>, (StatusCode, Json<BuscarFotoResponse>)> {
    let conn = pool.get().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BuscarFotoResponse {
                sucesso: false,
                mensagem: format!("Erro ao obter conexão: {}", e),
                foto_base64: None,
                foto_mime: None,
                origem: None,
            }),
        )
    })?;

    let _ = conn.execute(
        "UPDATE politicos SET foto_blob = NULL, foto_mime = NULL, foto_url = NULL WHERE id = ?1",
        [id],
    );

    Ok(Json(BuscarFotoResponse {
        sucesso: true,
        mensagem: "Foto removida com sucesso do banco de dados.".to_string(),
        foto_base64: None,
        foto_mime: None,
        origem: None,
    }))
}

/// Sincroniza fotos oficiais da Câmara dos Deputados para parlamentares cadastrados
pub async fn sincronizar_fotos_camara(pool: &DbPool) -> Result<usize, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| e.to_string())?;

    let mut total_atualizados = 0;
    for leg in [57, 56] {
        let url = format!("https://dadosabertos.camara.leg.br/api/v2/deputados?idLegislatura={}", leg);
        if let Ok(resp) = client.get(&url).header("User-Agent", "RadarCivico/0.1.0").send().await {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(dados) = json.get("dados").and_then(|d| d.as_array()) {
                        if let Ok(conn) = pool.get() {
                            for d in dados {
                                let nome = d.get("nome").and_then(|n| n.as_str()).unwrap_or("").trim();
                                let foto = d.get("urlFoto").and_then(|f| f.as_str()).unwrap_or("").trim();
                                if !nome.is_empty() && !foto.is_empty() {
                                    if let Ok(affected) = conn.execute(
                                        "UPDATE politicos SET foto_url = ?1 WHERE (UPPER(nome_urna) = UPPER(?2) OR UPPER(nome_completo) = UPPER(?2)) AND (foto_url IS NULL OR foto_url = '')",
                                        storage::rusqlite::params![foto, nome],
                                    ) {
                                        total_atualizados += affected;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(total_atualizados)
}

/// POST /api/politicos/sincronizar-fotos-camara e /api/v1/politicos/sincronizar-fotos-camara
pub async fn sincronizar_fotos_camara_handler(
    State(pool): State<DbPool>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match sincronizar_fotos_camara(&pool).await {
        Ok(count) => Ok(Json(serde_json::json!({
            "sucesso": true,
            "total_atualizados": count,
            "mensagem": format!("Sincronização concluída: {} parlamentares atualizados com foto oficial.", count)
        }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::{get, post};
    use axum::Router;
    use storage::run_migrations;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_politico_dossie_endpoint() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        let foto_bytes = vec![0x89, 0x50, 0x4E, 0x47];
        conn.execute(
            "INSERT INTO politicos (sq_candidato, cpf_mascarado, nome_completo, nome_urna, ocupacao, foto_blob, foto_mime)
             VALUES ('SQ9988', '***.555.666-**', 'MARCOS PONTE', 'ASTRONAUTA', 'ENGENHEIRO', ?1, 'image/png')",
            [&foto_bytes],
        ).unwrap();
        let pol_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf, total_bens_declarados)
             VALUES (?1, 2022, 'SENADOR', 'PL', 'SP', 2500000.0)",
            [pol_id],
        ).unwrap();
        let cand_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO bens_candidato (candidatura_id, tipo_bem, descricao, valor_declarado)
             VALUES (?1, 'IMOVEL', 'APARTAMENTO RESIDENCIAL', 1200000.0)",
            [cand_id],
        ).unwrap();

        conn.execute(
            "INSERT INTO receitas_campanha (candidatura_id, doador_cpf_cnpj, doador_nome, valor)
             VALUES (?1, '11122233344', 'DOADOR DESTAQUE', 50000.0)",
            [cand_id],
        ).unwrap();

        let app = Router::new()
            .route("/api/v1/politico/:id", get(politico_dossie_handler))
            .with_state(pool);

        let req = Request::builder()
            .uri(format!("/api/v1/politico/{}", pol_id))
            .body(Body::empty())
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let dossie: DossiePolitico = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(dossie.nome_completo, "MARCOS PONTE");
        assert_eq!(dossie.nome_urna, "ASTRONAUTA");
        assert!(dossie.foto_base64.is_some());
        assert_eq!(dossie.candidaturas.len(), 1);
        assert_eq!(dossie.historico_bens.len(), 1);
        assert_eq!(dossie.doadores.len(), 1);
    }

    #[tokio::test]
    async fn test_listar_e_detalhe_politicos_endpoints() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna, ocupacao)
             VALUES ('DENISE PESSÔA', 'DENISE PESSÔA', 'DEPUTADO FEDERAL')",
            [],
        ).unwrap();
        let pol_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf, municipio)
             VALUES (?1, 2022, 'DEPUTADO FEDERAL', 'PT', 'RS', 'Caxias do Sul')",
            [pol_id],
        ).unwrap();

        conn.execute(
            "INSERT INTO despesas_parlamentares (
                casa_legislativa, parlamentar_nome, data_emissao, categoria_despesa,
                fornecedor_nome, fornecedor_cnpj_cpf, valor_liquido
             ) VALUES ('CAMARA', 'DENISE PESSÔA', '2024-05-10', 'COMBUSTÍVEIS E LUBRIFICANTES.', '031 - 302 NORTE - CASCOL', '00306597003112', 250.0)",
            [],
        ).unwrap();

        let app = Router::new()
            .route("/api/politicos", get(listar_politicos_handler))
            .route("/api/politicos/:id", get(politico_detalhe_handler))
            .route("/api/politicos/:id/despesas-geo", get(politico_despesas_geo_handler))
            .with_state(pool);

        // 1. Testa listagem com filtro
        let req_list = Request::builder()
            .uri("/api/politicos?partido=PT&uf=RS")
            .body(Body::empty())
            .unwrap();
        let res_list = app.clone().oneshot(req_list).await.unwrap();
        assert_eq!(res_list.status(), StatusCode::OK);
        let list_bytes = axum::body::to_bytes(res_list.into_body(), usize::MAX).await.unwrap();
        let list_resp: ListarPoliticosResponse = serde_json::from_slice(&list_bytes).unwrap();
        assert_eq!(list_resp.total, 1);
        assert_eq!(list_resp.politicos[0].nome_completo, "DENISE PESSÔA");
        assert_eq!(list_resp.politicos[0].total_despesas_ceap, 250.0);

        // 2. Testa detalhe do político com resumo financeiro
        let req_detalhe = Request::builder()
            .uri(format!("/api/politicos/{}", pol_id))
            .body(Body::empty())
            .unwrap();
        let res_detalhe = app.clone().oneshot(req_detalhe).await.unwrap();
        assert_eq!(res_detalhe.status(), StatusCode::OK);
        let det_bytes = axum::body::to_bytes(res_detalhe.into_body(), usize::MAX).await.unwrap();
        let det_resp: PoliticoDetalheResponse = serde_json::from_slice(&det_bytes).unwrap();
        assert_eq!(det_resp.resumo_financeiro.total_gasto_ceap, 250.0);
        assert_eq!(det_resp.gastos_por_categoria.len(), 1);
        assert_eq!(det_resp.gastos_por_categoria[0].categoria, "COMBUSTÍVEIS E LUBRIFICANTES.");

        // 3. Testa georreferenciamento de despesas
        let req_geo = Request::builder()
            .uri(format!("/api/politicos/{}/despesas-geo", pol_id))
            .body(Body::empty())
            .unwrap();
        let res_geo = app.oneshot(req_geo).await.unwrap();
        assert_eq!(res_geo.status(), StatusCode::OK);
        let geo_bytes = axum::body::to_bytes(res_geo.into_body(), usize::MAX).await.unwrap();
        let geo_resp: PoliticoDespesasGeoResponse = serde_json::from_slice(&geo_bytes).unwrap();
        assert_eq!(geo_resp.total_despesas_geo, 1);
        assert_eq!(geo_resp.pontos[0].uf, "DF");
        assert_eq!(geo_resp.pontos[0].municipio, "Brasília");
    }

    #[tokio::test]
    async fn test_salvar_obter_e_buscar_foto_politico() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna) VALUES ('VEREADOR TESTE', 'VEREADOR DA SILVA')",
            [],
        ).unwrap();
        let pol_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf)
             VALUES (?1, 2024, 'VEREADOR', 'PL', 'SP')",
            [pol_id],
        ).unwrap();

        let app = Router::new()
            .route("/api/politicos/:id/foto", post(salvar_foto_manual_handler).get(obter_foto_handler).delete(remover_foto_handler))
            .route("/api/politicos/:id/buscar-foto-tse", post(buscar_foto_tse_handler))
            .route("/api/politicos", get(listar_politicos_handler))
            .with_state(pool.clone());

        // 1. Salva foto manual via base64
        let b64_fake = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let payload = serde_json::json!({
            "foto_base64": b64_fake,
            "foto_mime": "image/png"
        });

        let req_salvar = Request::builder()
            .uri(format!("/api/politicos/{}/foto", pol_id))
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&payload).unwrap()))
            .unwrap();

        let res_salvar = app.clone().oneshot(req_salvar).await.unwrap();
        assert_eq!(res_salvar.status(), StatusCode::OK);

        // 2. Obtém a foto binária
        let req_obter = Request::builder()
            .uri(format!("/api/politicos/{}/foto", pol_id))
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let res_obter = app.clone().oneshot(req_obter).await.unwrap();
        assert_eq!(res_obter.status(), StatusCode::OK);
        assert_eq!(res_obter.headers().get("content-type").unwrap(), "image/png");

        // 3. Testa buscar_foto_tse retornando foto já existente no banco de dados local
        let req_buscar = Request::builder()
            .uri(format!("/api/politicos/{}/buscar-foto-tse", pol_id))
            .method("POST")
            .body(Body::empty())
            .unwrap();

        let res_buscar = app.clone().oneshot(req_buscar).await.unwrap();
        assert_eq!(res_buscar.status(), StatusCode::OK);
        let buscar_bytes = axum::body::to_bytes(res_buscar.into_body(), usize::MAX).await.unwrap();
        let buscar_resp: BuscarFotoResponse = serde_json::from_slice(&buscar_bytes).unwrap();
        assert!(buscar_resp.sucesso);
        assert_eq!(buscar_resp.origem, Some("BANCO_LOCAL".to_string()));

        // 4. Testa listagem e verifica se mandatos contém VEREADOR (2024)
        let req_list = Request::builder()
            .uri("/api/politicos?cargo=VEREADOR")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let res_list = app.clone().oneshot(req_list).await.unwrap();
        assert_eq!(res_list.status(), StatusCode::OK);
        let list_bytes = axum::body::to_bytes(res_list.into_body(), usize::MAX).await.unwrap();
        let list_resp: ListarPoliticosResponse = serde_json::from_slice(&list_bytes).unwrap();
        assert_eq!(list_resp.total, 1);
        assert!(list_resp.politicos[0].mandatos.iter().any(|m| m.contains("VEREADOR")));
        assert!(list_resp.politicos[0].foto_base64.is_some());
    }

    #[tokio::test]
    async fn test_listar_politicos_filtro_ano_e_tabulacao() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // Político 1: concorreu em 2024 e 2020
        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna) VALUES ('CANDIDATO MULTIANO', 'MULTIANO')",
            [],
        ).unwrap();
        let p1_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf)
             VALUES (?1, 2024, 'PREFEITO', 'PSD', 'MG')",
            [p1_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf)
             VALUES (?1, 2020, 'VEREADOR', 'PSD', 'MG')",
            [p1_id],
        ).unwrap();

        // Político 2: concorreu apenas em 2022
        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna) VALUES ('DEPUTADO FEDERAL 2022', 'DEP 2022')",
            [],
        ).unwrap();
        let p2_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf)
             VALUES (?1, 2022, 'DEPUTADO FEDERAL', 'PT', 'SP')",
            [p2_id],
        ).unwrap();

        let app = Router::new()
            .route("/api/politicos", get(listar_politicos_handler))
            .with_state(pool);

        // 1. Sem filtro de ano: deve listar ambos e trazer anos_disponiveis ordenados decrescente
        let req_todos = Request::builder()
            .uri("/api/politicos")
            .body(Body::empty())
            .unwrap();
        let res_todos = app.clone().oneshot(req_todos).await.unwrap();
        assert_eq!(res_todos.status(), StatusCode::OK);
        let bytes_todos = axum::body::to_bytes(res_todos.into_body(), usize::MAX).await.unwrap();
        let resp_todos: ListarPoliticosResponse = serde_json::from_slice(&bytes_todos).unwrap();
        assert_eq!(resp_todos.total, 2);
        assert!(resp_todos.anos_disponiveis.contains(&2024));
        assert!(resp_todos.anos_disponiveis.contains(&2022));
        assert!(resp_todos.anos_disponiveis.contains(&2020));

        // 2. Filtro por ano 2022: deve retornar apenas DEP 2022
        let req_2022 = Request::builder()
            .uri("/api/politicos?ano=2022")
            .body(Body::empty())
            .unwrap();
        let res_2022 = app.clone().oneshot(req_2022).await.unwrap();
        assert_eq!(res_2022.status(), StatusCode::OK);
        let bytes_2022 = axum::body::to_bytes(res_2022.into_body(), usize::MAX).await.unwrap();
        let resp_2022: ListarPoliticosResponse = serde_json::from_slice(&bytes_2022).unwrap();
        assert_eq!(resp_2022.total, 1);
        assert_eq!(resp_2022.politicos[0].nome_urna, "DEP 2022");
        assert_eq!(resp_2022.politicos[0].ano_eleicao, Some(2022));

        // 3. Filtro por ano 2020: deve retornar MULTIANO e ter tabulação de mandatos
        let req_2020 = Request::builder()
            .uri("/api/politicos?ano=2020")
            .body(Body::empty())
            .unwrap();
        let res_2020 = app.clone().oneshot(req_2020).await.unwrap();
        assert_eq!(res_2020.status(), StatusCode::OK);
        let bytes_2020 = axum::body::to_bytes(res_2020.into_body(), usize::MAX).await.unwrap();
        let resp_2020: ListarPoliticosResponse = serde_json::from_slice(&bytes_2020).unwrap();
        assert_eq!(resp_2020.total, 1);
        assert_eq!(resp_2020.politicos[0].nome_urna, "MULTIANO");
    }
}
