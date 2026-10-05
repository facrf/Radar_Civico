use csv_async::AsyncReaderBuilder;
use futures::StreamExt;
use storage::rusqlite::Connection;
use storage::{batch_insert_receitas, NovaReceita};
use tokio::io::AsyncRead;

use crate::error::{IngestionError, Result};
use crate::normalizer::mascarar_cpf;

#[derive(Debug, Clone, PartialEq)]
pub struct CandidatoCsvRecord {
    pub ano_eleicao: i32,
    pub uf: String,
    pub cargo: String,
    pub sq_candidato: String,
    pub numero_urna: Option<i32>,
    pub nome_completo: String,
    pub nome_urna: String,
    pub cpf_mascarado: String,
    pub sigla_partido: String,
    pub municipio: Option<String>,
    pub situacao_totalizacao: Option<String>,
    pub ocupacao: Option<String>,
    pub grau_instrucao: Option<String>,
    pub data_nascimento: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReceitaCsvRecord {
    pub sq_candidato: Option<String>,
    pub doador_cpf_cnpj: String,
    pub doador_nome: String,
    pub valor: f64,
    pub data_receita: Option<String>,
    pub tipo_origem: Option<String>,
    pub descricao: Option<String>,
}

fn parse_float_br(val: &str) -> f64 {
    let clean = val.trim().replace('.', "").replace(',', ".");
    clean.parse::<f64>().unwrap_or(0.0)
}

pub async fn processar_stream_consulta_cand<R: AsyncRead + Unpin + Send>(
    reader: R,
) -> Result<Vec<CandidatoCsvRecord>> {
    let mut csv_reader = AsyncReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true)
        .create_reader(reader);

    let headers = csv_reader
        .headers()
        .await
        .map_err(|e| IngestionError::Csv(e))?
        .clone();

    let col_ano = headers.iter().position(|h| h.trim() == "ANO_ELEICAO");
    let col_uf = headers.iter().position(|h| h.trim() == "SG_UF");
    let col_cargo = headers.iter().position(|h| h.trim() == "DS_CARGO");
    let col_sq = headers.iter().position(|h| h.trim() == "SQ_CANDIDATO");
    let col_nr = headers.iter().position(|h| h.trim() == "NR_CANDIDATO");
    let col_nome = headers.iter().position(|h| h.trim() == "NM_CANDIDATO");
    let col_urna = headers.iter().position(|h| h.trim() == "NM_URNA_CANDIDATO");
    let col_cpf = headers.iter().position(|h| h.trim() == "NR_CPF_CANDIDATO");
    let col_partido = headers.iter().position(|h| h.trim() == "SG_PARTIDO");
    let col_municipio = headers.iter().position(|h| h.trim() == "NM_MUNICIPIO");
    let col_sit = headers.iter().position(|h| h.trim() == "DS_SIT_TOT_TURNO");
    let col_ocup = headers.iter().position(|h| h.trim() == "DS_OCUPACAO");
    let col_inst = headers.iter().position(|h| h.trim() == "DS_GRAU_INSTRUCAO");
    let col_nasc = headers.iter().position(|h| h.trim() == "DT_NASCIMENTO");

    let mut records = Vec::new();
    let mut records_stream = csv_reader.into_records();

    while let Some(record_res) = records_stream.next().await {
        let record = record_res.map_err(|e| IngestionError::Csv(e))?;

        let sq_candidato = col_sq
            .and_then(|idx| record.get(idx))
            .unwrap_or("")
            .trim()
            .to_string();

        if sq_candidato.is_empty() {
            continue;
        }

        let ano_eleicao = col_ano
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(0);

        let uf = col_uf
            .and_then(|idx| record.get(idx))
            .unwrap_or("")
            .trim()
            .to_string();

        let cargo = col_cargo
            .and_then(|idx| record.get(idx))
            .unwrap_or("")
            .trim()
            .to_string();

        let numero_urna = col_nr
            .and_then(|idx| record.get(idx))
            .and_then(|s| s.trim().parse::<i32>().ok());

        let nome_completo = col_nome
            .and_then(|idx| record.get(idx))
            .unwrap_or("")
            .trim()
            .to_string();

        let nome_urna = col_urna
            .and_then(|idx| record.get(idx))
            .unwrap_or("")
            .trim()
            .to_string();

        let raw_cpf = col_cpf.and_then(|idx| record.get(idx)).unwrap_or("");
        let cpf_mascarado = mascarar_cpf(raw_cpf);

        let sigla_partido = col_partido
            .and_then(|idx| record.get(idx))
            .unwrap_or("")
            .trim()
            .to_string();

        let municipio = col_municipio
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let situacao_totalizacao = col_sit
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let ocupacao = col_ocup
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let grau_instrucao = col_inst
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let data_nascimento = col_nasc
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        records.push(CandidatoCsvRecord {
            ano_eleicao,
            uf,
            cargo,
            sq_candidato,
            numero_urna,
            nome_completo,
            nome_urna,
            cpf_mascarado,
            sigla_partido,
            municipio,
            situacao_totalizacao,
            ocupacao,
            grau_instrucao,
            data_nascimento,
        });
    }

    Ok(records)
}

pub async fn processar_stream_receitas<R: AsyncRead + Unpin + Send>(
    reader: R,
) -> Result<Vec<ReceitaCsvRecord>> {
    let mut csv_reader = AsyncReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true)
        .create_reader(reader);

    let headers = csv_reader
        .headers()
        .await
        .map_err(|e| IngestionError::Csv(e))?
        .clone();

    let col_sq = headers.iter().position(|h| h.trim() == "SQ_CANDIDATO");
    let col_doc = headers
        .iter()
        .position(|h| h.trim() == "NR_CPF_CNPJ_DOADOR" || h.trim() == "NR_CPF_DOADOR");
    let col_nome = headers.iter().position(|h| h.trim() == "NM_DOADOR");
    let col_valor = headers.iter().position(|h| h.trim() == "VR_RECEITA");
    let col_data = headers.iter().position(|h| h.trim() == "DT_RECEITA");
    let col_origem = headers.iter().position(|h| h.trim() == "DS_ORIGEM_RECEITA");
    let col_desc = headers.iter().position(|h| h.trim() == "DS_RECEITA");

    let mut records = Vec::new();
    let mut records_stream = csv_reader.into_records();

    while let Some(record_res) = records_stream.next().await {
        let record = record_res.map_err(|e| IngestionError::Csv(e))?;

        let doador_nome = col_nome
            .and_then(|idx| record.get(idx))
            .unwrap_or("")
            .trim()
            .to_string();

        let doador_cpf_cnpj = col_doc
            .and_then(|idx| record.get(idx))
            .unwrap_or("")
            .trim()
            .to_string();

        let valor_str = col_valor.and_then(|idx| record.get(idx)).unwrap_or("0");
        let valor = parse_float_br(valor_str);

        let sq_candidato = col_sq
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let data_receita = col_data
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let tipo_origem = col_origem
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let descricao = col_desc
            .and_then(|idx| record.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        records.push(ReceitaCsvRecord {
            sq_candidato,
            doador_cpf_cnpj,
            doador_nome,
            valor,
            data_receita,
            tipo_origem,
            descricao,
        });
    }

    Ok(records)
}

pub async fn ingerir_receitas_tse_em_lotes<R: AsyncRead + Unpin + Send>(
    conn: &mut Connection,
    reader: R,
    candidatura_id: Option<i64>,
    batch_size: usize,
) -> Result<usize> {
    let receitas = processar_stream_receitas(reader).await?;
    let mut total_inserido = 0;

    for chunk in receitas.chunks(batch_size) {
        let batch: Vec<NovaReceita> = chunk
            .iter()
            .map(|r| NovaReceita {
                candidatura_id,
                doador_cpf_cnpj: r.doador_cpf_cnpj.clone(),
                doador_nome: r.doador_nome.clone(),
                valor: r.valor,
                data_receita: r.data_receita.clone(),
                tipo_origem: r.tipo_origem.clone(),
                descricao: r.descricao.clone(),
            })
            .collect();

        let inseridos = batch_insert_receitas(conn, &batch)?;
        total_inserido += inseridos;
    }

    Ok(total_inserido)
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{DbPool, run_migrations};

    #[tokio::test]
    async fn test_processar_stream_consulta_cand_tse() {
        let csv_data = "\
ANO_ELEICAO;SG_UF;DS_CARGO;SQ_CANDIDATO;NR_CANDIDATO;NM_CANDIDATO;NM_URNA_CANDIDATO;NR_CPF_CANDIDATO;SG_PARTIDO;NM_MUNICIPIO;DS_SIT_TOT_TURNO;DS_OCUPACAO;DS_GRAU_INSTRUCAO;DT_NASCIMENTO\n\
2024;SP;PREFEITO;250001234567;15;RICARDO NUNES;RICARDO NUNES;***123456**;MDB;SÃO PAULO;ELEITO;EMPRESÁRIO;SUPERIOR COMPLETO;13/11/1967\n\
2024;SP;PREFEITO;250007654321;50;GUILHERME BOULOS;GUILHERME BOULOS;***654321**;PSOL;SÃO PAULO;NÃO ELEITO;PROFESSOR;SUPERIOR COMPLETO;19/06/1982\n";

        let records = processar_stream_consulta_cand(csv_data.as_bytes()).await.unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].sq_candidato, "250001234567");
        assert_eq!(records[0].nome_urna, "RICARDO NUNES");
        assert_eq!(records[0].numero_urna, Some(15));
        assert_eq!(records[0].cpf_mascarado, "***.123.456-**");
        assert_eq!(records[0].situacao_totalizacao.as_deref(), Some("ELEITO"));

        assert_eq!(records[1].sq_candidato, "250007654321");
        assert_eq!(records[1].nome_urna, "GUILHERME BOULOS");
        assert_eq!(records[1].numero_urna, Some(50));
    }

    #[tokio::test]
    async fn test_processar_stream_e_ingerir_receitas_tse() {
        let csv_receitas = "\
SQ_CANDIDATO;NR_CPF_CNPJ_DOADOR;NM_DOADOR;VR_RECEITA;DT_RECEITA;DS_ORIGEM_RECEITA;DS_RECEITA\n\
250001234567;11122233344;DOADOR TESTE 1;1.500,50;2024-08-20;Doação PF;Transferência bancária\n\
250001234567;99888777000199;EMPRESA APOIADORA LTDA;25.000,00;2024-08-25;Fundo Partidário;Diretório Estadual\n";

        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        let total = ingerir_receitas_tse_em_lotes(&mut conn, csv_receitas.as_bytes(), None, 100)
            .await
            .unwrap();

        assert_eq!(total, 2);

        let count: i64 = conn
            .query_row("SELECT count(*) FROM receitas_campanha", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let soma: f64 = conn
            .query_row("SELECT sum(valor) FROM receitas_campanha", [], |r| r.get(0))
            .unwrap();
        assert!((soma - 26500.50).abs() < 0.01);
    }
}
