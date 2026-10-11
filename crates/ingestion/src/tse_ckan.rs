use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek};
use std::time::Duration;
use storage::rusqlite::Connection;
use storage::{
    batch_insert_bens_candidato, batch_insert_candidatos_tse, batch_insert_despesas,
    batch_insert_receitas, NovaDespesa, NovaReceita, NovoBemCandidato, NovoCandidatoTse,
};
use tokio::io::AsyncWriteExt;

use crate::error::{IngestionError, Result};
use crate::normalizer::{limpar_cnpj, mascarar_cpf};

pub const LOTE_BATCH_SIZE: usize = 25_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CkanResponse<T> {
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default)]
    pub success: bool,
    pub result: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CkanPackage {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub resources: Vec<CkanResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CkanResource {
    pub name: String,
    pub format: String,
    pub url: String,
    #[serde(default)]
    pub description: Option<String>,
}

fn parse_float_br(val: &str) -> f64 {
    let clean = val.trim().replace("R$", "").replace(' ', "");
    if clean.is_empty() {
        return 0.0;
    }
    if clean.contains(',') && clean.contains('.') {
        clean
            .replace('.', "")
            .replace(',', ".")
            .parse::<f64>()
            .unwrap_or(0.0)
    } else if clean.contains(',') {
        clean.replace(',', ".").parse::<f64>().unwrap_or(0.0)
    } else {
        clean.parse::<f64>().unwrap_or(0.0)
    }
}

pub fn selecionar_arquivos_zip(nomes: &[String]) -> Vec<String> {
    let mut brasil_files = Vec::new();
    let mut outros_csvs = Vec::new();

    for nome in nomes {
        let lower = nome.to_lowercase();
        if lower.ends_with(".csv") && !lower.contains("__macosx") {
            if lower.contains("rede_social") {
                continue;
            }
            if lower.contains("_brasil.csv") {
                brasil_files.push(nome.clone());
            } else {
                outros_csvs.push(nome.clone());
            }
        }
    }

    if !brasil_files.is_empty() {
        brasil_files
    } else {
        outros_csvs
    }
}

pub fn deve_ignorar_recurso_ckan(nome: &str, url: &str) -> bool {
    let lower_name = nome.to_lowercase();
    let lower_url = url.to_lowercase();
    let padroes = [
        "foto_cand",
        "fotos",
        "proposta_governo",
        "extrato_bancario",
        "fefc_",
        "certidao_criminal",
        "nota_fiscal",
        "rede_social",
        "complementar",
        "cnpj_campanha",
        "orgaos_partidarios",
        "coligacao",
        "vagas",
        "motivo_cassacao",
    ];
    for p in padroes {
        if lower_name.contains(p) || lower_url.contains(p) {
            return true;
        }
    }
    false
}

pub async fn descobrir_urls_tse(ano: u32, datasets: &[&str]) -> Result<Vec<String>> {
    descobrir_urls_tse_com_base("https://dadosabertos.tse.jus.br", ano, datasets).await
}

pub async fn descobrir_urls_tse_com_base(
    base_url: &str,
    ano: u32,
    datasets: &[&str],
) -> Result<Vec<String>> {
    let slugs = if datasets.is_empty() {
        vec![
            "candidatos",
            "prestacao-contas-eleitorais-candidatos",
            "bens-candidatos",
        ]
    } else {
        datasets.to_vec()
    };

    let client = Client::builder()
        .user_agent("RadarCivico/1.0 (Auditoria TSE CKAN)")
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    let mut urls = Vec::new();

    for slug in slugs {
        let package_id = format!("{}-{}", slug, ano);
        let url = format!(
            "{}/api/3/action/package_show?id={}",
            base_url.trim_end_matches('/'),
            package_id
        );

        let resp_res = client.get(&url).send().await;
        let resp = match resp_res {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Erro de requisição ao consultar CKAN para {package_id}: {e}");
                continue;
            }
        };

        if !resp.status().is_success() {
            // Suporte a alias caso seja prestação de contas na API real do TSE (ex: 2022 usa prefixo dadosabertos-tse-jus-br-dataset-)
            if slug == "prestacao-contas-eleitorais-candidatos" {
                let aliases = [
                    format!("prestacao-de-contas-eleitorais-{}", ano),
                    format!(
                        "dadosabertos-tse-jus-br-dataset-prestacao-de-contas-eleitorais-{}",
                        ano
                    ),
                ];
                for alias_id in &aliases {
                    let alias_url = format!(
                        "{}/api/3/action/package_show?id={}",
                        base_url.trim_end_matches('/'),
                        alias_id
                    );
                    if let Ok(alias_resp) = client.get(&alias_url).send().await {
                        if alias_resp.status().is_success() {
                            if let Ok(ckan_data) =
                                alias_resp.json::<CkanResponse<CkanPackage>>().await
                            {
                                for res in ckan_data.result.resources {
                                    if deve_ignorar_recurso_ckan(&res.name, &res.url) {
                                        continue;
                                    }
                                    let fmt = res.format.trim().to_uppercase();
                                    let u = res.url.trim();
                                    if (fmt == "ZIP" || u.to_lowercase().ends_with(".zip"))
                                        && !urls.contains(&res.url)
                                    {
                                        urls.push(res.url);
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
            continue;
        }

        if let Ok(ckan_data) = resp.json::<CkanResponse<CkanPackage>>().await {
            for res in ckan_data.result.resources {
                if deve_ignorar_recurso_ckan(&res.name, &res.url) {
                    continue;
                }
                let fmt = res.format.trim().to_uppercase();
                let u = res.url.trim();
                if (fmt == "ZIP" || u.to_lowercase().ends_with(".zip")) && !urls.contains(&res.url)
                {
                    urls.push(res.url);
                }
            }
        }
    }

    Ok(urls)
}

pub fn normalizar_cargo(cargo_raw: &str) -> String {
    let clean = cargo_raw.trim().to_uppercase();
    match clean.as_str() {
        "1" => "PRESIDENTE".to_string(),
        "2" => "VICE-PRESIDENTE".to_string(),
        "3" => "GOVERNADOR".to_string(),
        "4" => "VICE-GOVERNADOR".to_string(),
        "5" => "SENADOR".to_string(),
        "6" => "DEPUTADO FEDERAL".to_string(),
        "7" => "DEPUTADO ESTADUAL".to_string(),
        "8" => "DEPUTADO DISTRITAL".to_string(),
        "9" | "10" => "SUPLENTE".to_string(),
        "11" => "PREFEITO".to_string(),
        "12" => "VICE-PREFEITO".to_string(),
        "13" => "VEREADOR".to_string(),
        other => {
            if other.is_empty() || other == "CARGO" {
                "INDEFINIDO".to_string()
            } else {
                other.to_string()
            }
        }
    }
}

pub fn find_col_idx(headers: &csv::StringRecord, candidates: &[&str]) -> Option<usize> {
    // 1. Passada de correspondência EXATA, respeitando a ordem de preferência dos candidatos
    for &c in candidates {
        let c_norm = c.to_uppercase().replace(['"', '\'', '_', ' '], "");
        for (i, h) in headers.iter().enumerate() {
            let h_norm = h.trim().to_uppercase().replace(['"', '\'', '_', ' '], "");
            if h_norm == c_norm {
                return Some(i);
            }
        }
    }

    // 2. Prefixo/substring sem colidir CD_ (códigos) com descrições (DS_) ou siglas (SG_)
    for &c in candidates {
        let c_norm = c.to_uppercase().replace(['"', '\'', '_', ' '], "");
        for (i, h) in headers.iter().enumerate() {
            let h_norm = h.trim().to_uppercase().replace(['"', '\'', '_', ' '], "");
            if !h_norm.starts_with("CD") && h_norm.contains(&c_norm) {
                return Some(i);
            }
        }
    }

    // 3. Fallback genérico se nada mais casou
    for &c in candidates {
        let c_norm = c.to_uppercase().replace(['"', '\'', '_', ' '], "");
        for (i, h) in headers.iter().enumerate() {
            let h_norm = h.trim().to_uppercase().replace(['"', '\'', '_', ' '], "");
            if h_norm.contains(&c_norm) {
                return Some(i);
            }
        }
    }

    None
}

fn buscar_candidatura_id(
    conn: &Connection,
    cache: &mut std::collections::HashMap<String, Option<i64>>,
    sq: &str,
) -> Option<i64> {
    if sq.is_empty() {
        return None;
    }
    if let Some(&cached) = cache.get(sq) {
        return cached;
    }
    let res: Option<i64> = conn
        .query_row(
            "SELECT c.id FROM candidaturas c 
             JOIN politicos p ON c.politico_id = p.id 
             WHERE p.sq_candidato = ?1 LIMIT 1",
            [sq],
            |r| r.get(0),
        )
        .ok();
    if cache.len() >= 100_000 {
        cache.clear();
    }
    cache.insert(sq.to_string(), res);
    res
}

pub fn processar_csv_tse_str(
    conn: &mut Connection,
    csv_str: &str,
    batch_size: usize,
) -> Result<usize> {
    processar_csv_tse_str_com_progresso(conn, csv_str, batch_size, |n| {
        tracing::trace!(registros = n, "Lote TSE persistido")
    })
}

pub fn processar_csv_tse_str_com_progresso<F>(
    conn: &mut Connection,
    csv_str: &str,
    batch_size: usize,
    on_batch: F,
) -> Result<usize>
where
    F: FnMut(usize),
{
    processar_csv_tse_reader_com_progresso(conn, csv_str.as_bytes(), batch_size, on_batch)
}

pub fn processar_csv_tse_reader_com_progresso<R: Read, F: FnMut(usize)>(
    conn: &mut Connection,
    input: R,
    batch_size: usize,
    mut on_batch: F,
) -> Result<usize> {
    let batch_size = batch_size.clamp(1, LOTE_BATCH_SIZE);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true)
        .from_reader(input);

    let headers = reader
        .headers()
        .map_err(|e| IngestionError::Parse(format!("Erro ao ler headers CSV: {e}")))?
        .clone();

    let header_line = headers.iter().collect::<Vec<_>>().join(";").to_uppercase();

    let mut total_inseridos = 0;
    let mut sq_cache = std::collections::HashMap::<String, Option<i64>>::new();

    if (header_line.contains("VR_BEM")
        || header_line.contains("DS_TIPO_BEM")
        || header_line.contains("CD_TIPO_BEM"))
        && !header_line.contains("NM_CANDIDATO")
    {
        // 1. Bens de Candidatos
        let col_tipo = find_col_idx(
            &headers,
            &["DS_TIPO_BEM_CANDIDATO", "CD_TIPO_BEM_CANDIDATO", "TIPO_BEM"],
        );
        let col_desc = find_col_idx(&headers, &["DS_BEM_CANDIDATO", "DESCRICAO", "DETALHE_BEM"]);
        let col_valor = find_col_idx(&headers, &["VR_BEM_CANDIDATO", "VALOR_BEM", "VALOR"]);
        let col_sq = find_col_idx(&headers, &["SQ_CANDIDATO"]);

        let mut batch: Vec<NovoBemCandidato> = Vec::with_capacity(batch_size);

        for record in reader.records() {
            let record = record
                .map_err(|e| IngestionError::Parse(format!("Registro CSV TSE inválido: {e}")))?;
            let tipo = col_tipo
                .and_then(|i| record.get(i))
                .unwrap_or("OUTROS BENS")
                .trim()
                .to_string();
            let desc = col_desc
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let valor = col_valor
                .and_then(|i| record.get(i))
                .map(parse_float_br)
                .unwrap_or(0.0);

            let sq = col_sq.and_then(|i| record.get(i)).unwrap_or("").trim();
            let cand_id = buscar_candidatura_id(conn, &mut sq_cache, sq);

            batch.push(NovoBemCandidato {
                candidatura_id: cand_id,
                tipo_bem: tipo,
                descricao: desc,
                valor_declarado: valor,
            });

            if batch.len() >= batch_size {
                let n = batch_insert_bens_candidato(conn, &batch)?;
                total_inseridos += n;
                on_batch(n);
                batch.clear();
            }
        }

        if !batch.is_empty() {
            let n = batch_insert_bens_candidato(conn, &batch)?;
            total_inseridos += n;
            on_batch(n);
        }
    } else if header_line.contains("VR_RECEITA")
        || (header_line.contains("DOADOR") && header_line.contains("VALOR"))
    {
        // 2. Receitas de Campanha
        let col_doc = find_col_idx(
            &headers,
            &["NR_CPF_CNPJ_DOADOR", "DOADOR_CPF_CNPJ", "CPF_CNPJ_DOADOR"],
        );
        let col_nome = find_col_idx(&headers, &["NM_DOADOR", "DOADOR_NOME", "NOME_DOADOR"]);
        let col_valor = find_col_idx(&headers, &["VR_RECEITA", "VALOR_RECEITA", "VALOR"]);
        let col_data = find_col_idx(&headers, &["DT_RECEITA", "DATA_RECEITA", "DATA"]);
        let col_origem = find_col_idx(&headers, &["DS_ORIGEM_RECEITA", "ORIGEM", "TIPO_ORIGEM"]);
        let col_desc = find_col_idx(&headers, &["DS_RECEITA", "DESCRICAO"]);
        let col_sq = find_col_idx(&headers, &["SQ_CANDIDATO"]);

        let mut batch: Vec<NovaReceita> = Vec::with_capacity(batch_size);

        for record in reader.records() {
            let record = record
                .map_err(|e| IngestionError::Parse(format!("Registro CSV TSE inválido: {e}")))?;
            let doc_raw = col_doc
                .and_then(|i| record.get(i))
                .unwrap_or("00000000000")
                .trim();
            let doc = limpar_cnpj(doc_raw);
            let nome = col_nome
                .and_then(|i| record.get(i))
                .unwrap_or("DOADOR")
                .trim()
                .to_string();
            let valor = col_valor
                .and_then(|i| record.get(i))
                .map(parse_float_br)
                .unwrap_or(0.0);
            let data = col_data
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let origem = col_origem
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let desc = col_desc
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let sq = col_sq.and_then(|i| record.get(i)).unwrap_or("").trim();
            let cand_id = buscar_candidatura_id(conn, &mut sq_cache, sq);

            if !doc.is_empty() {
                batch.push(NovaReceita {
                    candidatura_id: cand_id,
                    doador_cpf_cnpj: doc,
                    doador_nome: nome,
                    valor,
                    data_receita: data,
                    tipo_origem: origem,
                    descricao: desc,
                });
            }

            if batch.len() >= batch_size {
                let n = batch_insert_receitas(conn, &batch)?;
                total_inseridos += n;
                on_batch(n);
                batch.clear();
            }
        }

        if !batch.is_empty() {
            let n = batch_insert_receitas(conn, &batch)?;
            total_inseridos += n;
            on_batch(n);
        }
    } else if header_line.contains("VR_DESPESA")
        || (header_line.contains("FORNECEDOR") && header_line.contains("DESPESA"))
    {
        // 3. Despesas de Campanha
        let col_doc = find_col_idx(
            &headers,
            &[
                "NR_CPF_CNPJ_FORNECEDOR",
                "FORNECEDOR_CPF_CNPJ",
                "CPF_CNPJ_FORNECEDOR",
            ],
        );
        let col_nome = find_col_idx(
            &headers,
            &["NM_FORNECEDOR", "FORNECEDOR_NOME", "NOME_FORNECEDOR"],
        );
        let col_valor = find_col_idx(&headers, &["VR_DESPESA", "VALOR_DESPESA", "VALOR"]);
        let col_data = find_col_idx(&headers, &["DT_DESPESA", "DATA_DESPESA", "DATA"]);
        let col_tipo = find_col_idx(
            &headers,
            &["DS_DESPESA", "TIPO_DESPESA", "DS_ORIGEM_DESPESA"],
        );
        let col_desc = find_col_idx(&headers, &["DS_DESPESA", "DESCRICAO"]);
        let col_sq = find_col_idx(&headers, &["SQ_CANDIDATO"]);

        let mut batch: Vec<NovaDespesa> = Vec::with_capacity(batch_size);

        for record in reader.records() {
            let record = record
                .map_err(|e| IngestionError::Parse(format!("Registro CSV TSE inválido: {e}")))?;
            let doc_raw = col_doc
                .and_then(|i| record.get(i))
                .unwrap_or("00000000000100")
                .trim();
            let doc = limpar_cnpj(doc_raw);
            let nome = col_nome
                .and_then(|i| record.get(i))
                .unwrap_or("FORNECEDOR")
                .trim()
                .to_string();
            let valor = col_valor
                .and_then(|i| record.get(i))
                .map(parse_float_br)
                .unwrap_or(0.0);
            let data = col_data
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let tipo = col_tipo
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let desc = col_desc
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let sq = col_sq.and_then(|i| record.get(i)).unwrap_or("").trim();
            let cand_id = buscar_candidatura_id(conn, &mut sq_cache, sq);

            if !doc.is_empty() {
                batch.push(NovaDespesa {
                    candidatura_id: cand_id,
                    fornecedor_cpf_cnpj: doc,
                    fornecedor_nome: nome,
                    valor,
                    data_despesa: data,
                    tipo_despesa: tipo,
                    descricao: desc,
                });
            }

            if batch.len() >= batch_size {
                let n = batch_insert_despesas(conn, &batch)?;
                total_inseridos += n;
                on_batch(n);
                batch.clear();
            }
        }

        if !batch.is_empty() {
            let n = batch_insert_despesas(conn, &batch)?;
            total_inseridos += n;
            on_batch(n);
        }
    } else if (header_line.contains("NM_CANDIDATO") || header_line.contains("NOME_COMPLETO"))
        && (header_line.contains("NR_CPF_CANDIDATO") || header_line.contains("SQ_CANDIDATO"))
    {
        // 4. Candidatos (consulta_cand)
        let col_ano = find_col_idx(&headers, &["ANO_ELEICAO"]);
        let col_uf = find_col_idx(&headers, &["SG_UF", "UF"]);
        let col_cargo = find_col_idx(&headers, &["DS_CARGO", "CARGO"]);
        let col_sq = find_col_idx(&headers, &["SQ_CANDIDATO"]);
        let col_nr = find_col_idx(&headers, &["NR_CANDIDATO", "NUMERO_URNA"]);
        let col_nome = find_col_idx(&headers, &["NM_CANDIDATO", "NOME_COMPLETO"]);
        let col_urna = find_col_idx(&headers, &["NM_URNA_CANDIDATO", "NOME_URNA"]);
        let col_cpf = find_col_idx(&headers, &["NR_CPF_CANDIDATO", "CPF"]);
        let col_partido = find_col_idx(&headers, &["SG_PARTIDO", "PARTIDO"]);
        let col_municipio = find_col_idx(&headers, &["NM_MUNICIPIO", "MUNICIPIO"]);
        let col_sit = find_col_idx(&headers, &["DS_SIT_TOT_TURNO", "SITUACAO_TOTALIZACAO"]);
        let col_ocup = find_col_idx(&headers, &["DS_OCUPACAO", "OCUPACAO"]);
        let col_inst = find_col_idx(&headers, &["DS_GRAU_INSTRUCAO", "GRAU_INSTRUCAO"]);
        let col_nasc = find_col_idx(&headers, &["DT_NASCIMENTO", "DATA_NASCIMENTO"]);

        let mut batch: Vec<NovoCandidatoTse> = Vec::with_capacity(batch_size);

        for record in reader.records() {
            let record = record
                .map_err(|e| IngestionError::Parse(format!("Registro CSV TSE inválido: {e}")))?;
            let sq = col_sq.and_then(|i| record.get(i)).unwrap_or("").trim();
            if sq.is_empty() || sq.eq_ignore_ascii_case("SQ_CANDIDATO") {
                continue;
            }

            let ano = col_ano
                .and_then(|i| record.get(i))
                .and_then(|s| s.trim().parse::<i32>().ok())
                .unwrap_or(2024);
            let uf = col_uf
                .and_then(|i| record.get(i))
                .unwrap_or("BR")
                .trim()
                .to_string();
            let raw_cargo = col_cargo.and_then(|i| record.get(i)).unwrap_or("").trim();
            let cargo = normalizar_cargo(raw_cargo);
            if cargo == "INDEFINIDO" {
                continue;
            }
            let nr = col_nr
                .and_then(|i| record.get(i))
                .and_then(|s| s.trim().parse::<i32>().ok());
            let nome = col_nome
                .and_then(|i| record.get(i))
                .unwrap_or("CANDIDATO")
                .trim()
                .to_string();
            let urna = col_urna
                .and_then(|i| record.get(i))
                .unwrap_or(&nome)
                .trim()
                .to_string();
            let raw_cpf = col_cpf.and_then(|i| record.get(i)).unwrap_or("").trim();
            let cpf = mascarar_cpf(raw_cpf);
            let partido = col_partido
                .and_then(|i| record.get(i))
                .unwrap_or("PARTIDO")
                .trim()
                .to_string();
            let municipio = col_municipio
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let sit = col_sit
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let ocup = col_ocup
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let inst = col_inst
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let nasc = col_nasc
                .and_then(|i| record.get(i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            batch.push(NovoCandidatoTse {
                sq_candidato: sq.to_string(),
                cpf_mascarado: cpf,
                nome_completo: nome,
                nome_urna: urna,
                data_nascimento: nasc,
                grau_instrucao: inst,
                ocupacao: ocup,
                ano_eleicao: ano,
                cargo,
                numero_urna: nr,
                sigla_partido: partido,
                uf,
                municipio,
                situacao_totalizacao: sit,
            });

            if batch.len() >= batch_size {
                let n = batch_insert_candidatos_tse(conn, &batch)?;
                total_inseridos += n;
                on_batch(n);
                batch.clear();
            }
        }

        if !batch.is_empty() {
            let n = batch_insert_candidatos_tse(conn, &batch)?;
            total_inseridos += n;
            on_batch(n);
        }
    } else {
        tracing::info!("CSV ignorado por não corresponder aos schemas suportados (candidatos, receitas, despesas, bens)");
    }

    Ok(total_inseridos)
}

/// Limites de disco/descompressão; memória permanece limitada ao buffer e ao lote.
pub const MAX_DOWNLOAD_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_ZIP_ENTRY_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub const MAX_ZIP_TOTAL_BYTES: u64 = 64 * 1024 * 1024 * 1024;

pub struct LimitedRead<R> {
    inner: R,
    remaining: u64,
}
impl<R: Read> LimitedRead<R> {
    pub fn new(inner: R, maximum: u64) -> Self {
        Self {
            inner,
            remaining: maximum,
        }
    }
}
impl<R: Read> Read for LimitedRead<R> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut probe = [0u8; 1];
            if self.inner.read(&mut probe)? > 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Limite de descompressão excedido",
                ));
            }
            return Ok(0);
        }
        let length = output
            .len()
            .min(self.remaining.min(usize::MAX as u64) as usize);
        let read = self.inner.read(&mut output[..length])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}

/// Converte Windows-1252/Latin-1 em blocos, sem materializar o CSV inteiro.
struct Latin1Reader<R> {
    inner: R,
    decoded: Vec<u8>,
    position: usize,
}
impl<R: Read> Latin1Reader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            decoded: Vec::new(),
            position: 0,
        }
    }
}
impl<R: Read> Read for Latin1Reader<R> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.position == self.decoded.len() {
            let mut input = [0u8; 8192];
            let count = self.inner.read(&mut input)?;
            if count == 0 {
                return Ok(0);
            }
            let (text, _, _) = encoding_rs::WINDOWS_1252.decode(&input[..count]);
            self.decoded.clear();
            self.decoded.extend_from_slice(text.as_bytes());
            self.position = 0;
        }
        let count = output.len().min(self.decoded.len() - self.position);
        output[..count].copy_from_slice(&self.decoded[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}

pub fn processar_zip_tse_reader<R: Read + Seek, F: FnMut(&str, usize)>(
    conn: &mut Connection,
    reader: R,
    mut on_batch: F,
) -> Result<usize> {
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| IngestionError::Zip(format!("Erro ao abrir ZIP TSE: {e}")))?;
    if archive.len() > 10_000 {
        return Err(IngestionError::Zip("ZIP contém arquivos demais".into()));
    }
    let mut names = Vec::new();
    let mut total_bytes = 0u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| IngestionError::Zip(e.to_string()))?;
        total_bytes = total_bytes
            .checked_add(entry.size())
            .ok_or_else(|| IngestionError::Zip("Tamanho ZIP inválido".into()))?;
        if entry.size() > MAX_ZIP_ENTRY_BYTES || total_bytes > MAX_ZIP_TOTAL_BYTES {
            return Err(IngestionError::Zip(
                "ZIP excede limite de descompressão".into(),
            ));
        }
        names.push(entry.name().to_string());
    }
    let mut total = 0;
    for name in selecionar_arquivos_zip(&names) {
        let entry = archive
            .by_name(&name)
            .map_err(|e| IngestionError::Zip(e.to_string()))?;
        let input = Latin1Reader::new(LimitedRead::new(entry, MAX_ZIP_ENTRY_BYTES));
        total += processar_csv_tse_reader_com_progresso(conn, input, LOTE_BATCH_SIZE, |n| {
            on_batch(&name, n)
        })?;
    }
    Ok(total)
}

pub fn processar_zip_tse_bytes(conn: &mut Connection, zip_bytes: &[u8]) -> Result<usize> {
    processar_zip_tse_reader(conn, std::io::Cursor::new(zip_bytes), |name, n| {
        tracing::trace!(arquivo = name, registros = n, "Lote TSE persistido")
    })
}

pub fn processar_zip_tse_bytes_com_progresso(
    conn: &mut Connection,
    zip_bytes: &[u8],
    nome_pacote: &str,
    indice_pacote: usize,
    total_pacotes: usize,
    start_time: std::time::Instant,
) -> Result<usize> {
    processar_zip_tse_reader(conn, std::io::Cursor::new(zip_bytes), |name, n| {
        crate::progress::update_import_progress_batch(
            &format!("{nome_pacote} -> {name}"),
            indice_pacote,
            total_pacotes,
            n as u64,
            start_time,
        );
    })
}

pub async fn baixar_pacote_tse(client: &Client, url: &str) -> Result<tempfile::NamedTempFile> {
    let response = client
        .get(url)
        .timeout(Duration::from_secs(120))
        .send()
        .await?
        .error_for_status()?;
    if response
        .content_length()
        .is_some_and(|n| n > MAX_DOWNLOAD_BYTES)
    {
        return Err(IngestionError::Zip(
            "Download TSE excede limite de tamanho".into(),
        ));
    }
    let temporary = tempfile::NamedTempFile::new()?;
    let mut file = tokio::fs::File::from_std(temporary.reopen()?);
    let mut stream = response.bytes_stream();
    let mut total = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        total += chunk.len() as u64;
        if total > MAX_DOWNLOAD_BYTES {
            return Err(IngestionError::Zip(
                "Download TSE excede limite de tamanho".into(),
            ));
        }
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(temporary)
}

pub async fn baixar_e_processar_pacote_tse(conn: &mut Connection, url: &str) -> Result<usize> {
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;
    let file = baixar_pacote_tse(&client, url).await?;
    processar_zip_tse_reader(conn, file.reopen()?, |name, n| {
        tracing::trace!(arquivo = name, registros = n, "Lote TSE persistido")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use storage::{run_migrations, DbPool};

    #[test]
    fn test_tse_ckan_deserializacao_resposta() {
        let mock_json = r#"{
            "help": "https://dadosabertos.tse.jus.br/help",
            "success": true,
            "result": {
                "id": "candidatos-2024",
                "name": "candidatos-2024",
                "title": "Candidatos - 2024",
                "resources": [
                    {
                        "name": "Candidatos (Brasil)",
                        "format": "ZIP",
                        "url": "https://cdn.tse.jus.br/estatistica/sead/odsele/consulta_cand/consulta_cand_2024.zip"
                    },
                    {
                        "name": "Dicionário de Dados",
                        "format": "PDF",
                        "url": "https://cdn.tse.jus.br/docs/dicionario.pdf"
                    }
                ]
            }
        }"#;

        let resp: CkanResponse<CkanPackage> = serde_json::from_str(mock_json).unwrap();
        assert!(resp.success);
        assert_eq!(resp.result.name, Some("candidatos-2024".to_string()));
        assert_eq!(resp.result.resources.len(), 2);
        assert_eq!(resp.result.resources[0].format, "ZIP");
    }

    #[test]
    fn test_tse_ckan_selecionar_arquivos_prioriza_brasil() {
        let arquivos_com_brasil = vec![
            "consulta_cand_2024_SP.csv".to_string(),
            "consulta_cand_2024_RJ.csv".to_string(),
            "consulta_cand_2024_BRASIL.csv".to_string(),
            "dicionario.pdf".to_string(),
        ];

        let selecionados = selecionar_arquivos_zip(&arquivos_com_brasil);
        assert_eq!(
            selecionados,
            vec!["consulta_cand_2024_BRASIL.csv".to_string()]
        );

        let arquivos_sem_brasil = vec![
            "consulta_cand_2024_AC.csv".to_string(),
            "consulta_cand_2024_AL.csv".to_string(),
            "leia_me.txt".to_string(),
        ];

        let selecionados_estados = selecionar_arquivos_zip(&arquivos_sem_brasil);
        assert_eq!(
            selecionados_estados,
            vec![
                "consulta_cand_2024_AC.csv".to_string(),
                "consulta_cand_2024_AL.csv".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn test_tse_ckan_descobrir_urls_mock_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            loop {
                if let Ok((mut socket, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        use tokio::io::{AsyncReadExt, AsyncWriteExt};
                        let mut buf = [0u8; 2048];
                        let _ = socket.read(&mut buf).await;

                        let body = serde_json::json!({
                            "help": "mock",
                            "success": true,
                            "result": {
                                "id": "candidatos-2024",
                                "resources": [
                                    {
                                        "name": "Candidatos ZIP",
                                        "format": "ZIP",
                                        "url": "https://cdn.tse.jus.br/consulta_cand_2024.zip"
                                    },
                                    {
                                        "name": "Bens ZIP",
                                        "format": "CSV",
                                        "url": "https://cdn.tse.jus.br/bem_candidato_2024.zip"
                                    },
                                    {
                                        "name": "Manual",
                                        "format": "PDF",
                                        "url": "https://cdn.tse.jus.br/manual.pdf"
                                    }
                                ]
                            }
                        });

                        let body_str = serde_json::to_string(&body).unwrap();
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body_str.len(),
                            body_str
                        );
                        let _ = socket.write_all(resp.as_bytes()).await;
                    });
                }
            }
        });

        let base_url = format!("http://127.0.0.1:{}", port);
        let urls = descobrir_urls_tse_com_base(&base_url, 2024, &["candidatos"])
            .await
            .unwrap();

        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"https://cdn.tse.jus.br/consulta_cand_2024.zip".to_string()));
        assert!(urls.contains(&"https://cdn.tse.jus.br/bem_candidato_2024.zip".to_string()));
    }

    #[test]
    fn test_tse_ckan_processar_zip_seletivo_com_batch_5000() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // CSV com acentuação ISO-8859-1 (ex: SÃO PAULO, JOSÉ, EMPRESÁRIO)
        let csv_brasil_utf8 = "\
ANO_ELEICAO;SG_UF;DS_CARGO;SQ_CANDIDATO;NR_CANDIDATO;NM_CANDIDATO;NM_URNA_CANDIDATO;NR_CPF_CANDIDATO;SG_PARTIDO;NM_MUNICIPIO;DS_SIT_TOT_TURNO;DS_OCUPACAO;DS_GRAU_INSTRUCAO;DT_NASCIMENTO\n\
2024;SP;PREFEITO;250001;15;JOSÉ DA SILVA;JOSÉ;11122233344;MDB;SÃO PAULO;ELEITO;EMPRESÁRIO;SUPERIOR COMPLETO;10/05/1975\n\
2024;RJ;VEREADOR;250002;20;MARIA DAS GRAÇAS;MARIA;55566677788;PL;RIO DE JANEIRO;SUPLENTE;MÉDICA;SUPERIOR COMPLETO;20/08/1980\n";

        // Converte para Windows-1252 / ISO-8859-1
        let (bytes_latin1, _, _) = encoding_rs::WINDOWS_1252.encode(csv_brasil_utf8);

        // Cria ZIP contendo _BRASIL.csv e um arquivo de estado _SP.csv que deve ser ignorado pela priorização
        let mut zip_buffer = Vec::new();
        {
            let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buffer));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            zip_writer
                .start_file("consulta_cand_2024_SP.csv", options)
                .unwrap();
            zip_writer.write_all(b"IGNORADO").unwrap();

            zip_writer
                .start_file("consulta_cand_2024_BRASIL.csv", options)
                .unwrap();
            zip_writer.write_all(&bytes_latin1).unwrap();

            zip_writer.finish().unwrap();
        }

        let inseridos = processar_zip_tse_bytes(&mut conn, &zip_buffer).unwrap();
        assert_eq!(inseridos, 2);

        let total_politicos: i64 = conn
            .query_row("SELECT count(*) FROM politicos", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total_politicos, 2);

        let total_candidaturas: i64 = conn
            .query_row("SELECT count(*) FROM candidaturas", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total_candidaturas, 2);

        // Valida decodificação de acentos sem corromper
        let (nome, ocupacao, municipio): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT p.nome_completo, p.ocupacao, c.municipio 
                 FROM politicos p JOIN candidaturas c ON c.politico_id = p.id 
                 WHERE p.sq_candidato = '250001'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();

        assert_eq!(nome, "JOSÉ DA SILVA");
        assert_eq!(ocupacao, Some("EMPRESÁRIO".to_string()));
        assert_eq!(municipio, Some("SÃO PAULO".to_string()));
    }

    #[test]
    fn test_tse_ckan_processar_zip_bens_e_receitas() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        let csv_bens = "\
SQ_CANDIDATO;DS_TIPO_BEM_CANDIDATO;DS_BEM_CANDIDATO;VR_BEM_CANDIDATO\n\
250001;VEÍCULO AUTOMOTOR;VEÍCULO FIAT TORO 2022;120.000,00\n\
250001;APARTAMENTO;APARTAMENTO EM SÃO PAULO;450.000,00\n";

        let (bytes_latin1, _, _) = encoding_rs::WINDOWS_1252.encode(csv_bens);

        let mut zip_buffer = Vec::new();
        {
            let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buffer));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            zip_writer
                .start_file("bem_candidato_2024_BRASIL.csv", options)
                .unwrap();
            zip_writer.write_all(&bytes_latin1).unwrap();

            zip_writer.finish().unwrap();
        }

        let inseridos = processar_zip_tse_bytes(&mut conn, &zip_buffer).unwrap();
        assert_eq!(inseridos, 2);

        let count_bens: i64 = conn
            .query_row("SELECT count(*) FROM bens_candidato", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count_bens, 2);

        let valor_total: f64 = conn
            .query_row("SELECT sum(valor_declarado) FROM bens_candidato", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(valor_total, 570000.0);
    }

    #[test]
    fn test_tse_ckan_filtro_recursos_ckan_e_redes_sociais() {
        assert!(deve_ignorar_recurso_ckan(
            "Fotos dos Candidatos SP",
            "https://cdn.tse.jus.br/foto_cand2024_SP.zip"
        ));
        assert!(deve_ignorar_recurso_ckan(
            "Proposta de Governo",
            "https://cdn.tse.jus.br/proposta_governo_2024.zip"
        ));
        assert!(deve_ignorar_recurso_ckan(
            "Extrato Bancario",
            "https://cdn.tse.jus.br/extrato_bancario_2024.zip"
        ));
        assert!(deve_ignorar_recurso_ckan(
            "FEFC",
            "https://cdn.tse.jus.br/fefc_2024.zip"
        ));
        assert!(!deve_ignorar_recurso_ckan(
            "Consulta Cand Brasil",
            "https://cdn.tse.jus.br/consulta_cand_2024.zip"
        ));

        let arquivos = vec![
            "rede_social_candidato_2024_BRASIL.csv".to_string(),
            "consulta_cand_2024_BRASIL.csv".to_string(),
        ];
        let selecionados = selecionar_arquivos_zip(&arquivos);
        assert_eq!(
            selecionados,
            vec!["consulta_cand_2024_BRASIL.csv".to_string()]
        );
    }

    #[test]
    fn test_tse_ckan_ignora_csv_schema_nao_mapeado() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // CSV de redes sociais não possui colunas obrigatórias de candidatos
        let csv_redes = "\
ANO_ELEICAO;CD_TIPO_ELEICAO;NM_TIPO_ELEICAO;CD_ELEICAO;DS_ELEICAO;DT_ELEICAO;SG_UF;SG_UE;NM_UE;SQ_CANDIDATO;NR_ORDEM_REDE_SOCIAL;DS_URL\n\
2024;2;ELEICAO ORDINARIA;619;ELEICOES MUNICIPAIS 2024;06/10/2024;SP;71072;SAO PAULO;250001;1;https://instagram.com/teste\n";

        let inseridos = processar_csv_tse_str(&mut conn, csv_redes, 1000).unwrap();
        assert_eq!(inseridos, 0);

        let total_politicos: i64 = conn
            .query_row("SELECT count(*) FROM politicos", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total_politicos, 0);
    }

    #[test]
    fn test_normalizar_cargo() {
        assert_eq!(normalizar_cargo("1"), "PRESIDENTE");
        assert_eq!(normalizar_cargo("2"), "VICE-PRESIDENTE");
        assert_eq!(normalizar_cargo("3"), "GOVERNADOR");
        assert_eq!(normalizar_cargo("4"), "VICE-GOVERNADOR");
        assert_eq!(normalizar_cargo("5"), "SENADOR");
        assert_eq!(normalizar_cargo("6"), "DEPUTADO FEDERAL");
        assert_eq!(normalizar_cargo("7"), "DEPUTADO ESTADUAL");
        assert_eq!(normalizar_cargo("8"), "DEPUTADO DISTRITAL");
        assert_eq!(normalizar_cargo("11"), "PREFEITO");
        assert_eq!(normalizar_cargo("12"), "VICE-PREFEITO");
        assert_eq!(normalizar_cargo("13"), "VEREADOR");
        assert_eq!(normalizar_cargo("PRESIDENTE"), "PRESIDENTE");
        assert_eq!(normalizar_cargo("GOVERNADOR"), "GOVERNADOR");
        assert_eq!(normalizar_cargo("CARGO"), "INDEFINIDO");
        assert_eq!(normalizar_cargo(""), "INDEFINIDO");
    }

    #[test]
    fn test_find_col_idx_prioriza_exato_sobre_codigo() {
        let headers = csv::StringRecord::from(vec![
            "CD_CARGO",
            "DS_CARGO",
            "CD_PARTIDO",
            "SG_PARTIDO",
            "CD_SIT_TOT_TURNO",
            "DS_SIT_TOT_TURNO",
        ]);
        assert_eq!(find_col_idx(&headers, &["DS_CARGO", "CARGO"]), Some(1));
        assert_eq!(find_col_idx(&headers, &["SG_PARTIDO", "PARTIDO"]), Some(3));
        assert_eq!(
            find_col_idx(&headers, &["DS_SIT_TOT_TURNO", "SITUACAO_TOTALIZACAO"]),
            Some(5)
        );
    }
}

#[cfg(test)]
mod streaming_regressions {
    use super::*;
    #[test]
    fn leitor_limitado_rejeita_excesso_sem_truncar_silenciosamente() {
        let mut reader = LimitedRead::new(std::io::Cursor::new(b"12345"), 4);
        let mut output = [0u8; 32];
        assert_eq!(reader.read(&mut output).unwrap(), 4);
        assert!(reader.read(&mut output).is_err());
        let mut exact = LimitedRead::new(std::io::Cursor::new(b"1234"), 4);
        assert_eq!(exact.read(&mut output).unwrap(), 4);
        assert_eq!(exact.read(&mut output).unwrap(), 0);
    }
    #[test]
    fn latin1_preserva_acentos_entre_blocos_e_csv_persiste_em_lotes() {
        let text = format!("{}José;São Paulo\n", "x".repeat(8191));
        let (latin1, _, _) = encoding_rs::WINDOWS_1252.encode(&text);
        let mut reader = Latin1Reader::new(std::io::Cursor::new(latin1));
        let mut decoded = String::new();
        reader.read_to_string(&mut decoded).unwrap();
        assert_eq!(decoded, text);
        let pool = storage::DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        storage::run_migrations(&mut conn).unwrap();
        let csv = "NM_DOADOR;NR_CPF_CNPJ_DOADOR;VR_RECEITA\nJOSE;12345678900;10,00\nMARIA;98765432100;20,00\nANA;55566677788;30,00\n";
        let mut batches = Vec::new();
        let count = processar_csv_tse_reader_com_progresso(&mut conn, csv.as_bytes(), 2, |n| {
            batches.push(n)
        })
        .unwrap();
        assert_eq!(count, 3);
        assert_eq!(batches, [2, 1]);
    }
}
