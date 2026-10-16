//! Avaliação compartilhada pela listagem e pelos dois dossiês.
use storage::{rusqlite::Connection, AvaliacaoPolitico, Result};

pub fn avaliar(conn: &Connection, id: i64) -> Result<AvaliacaoPolitico> {
    let mut evaluation = storage::avaliacao_politico(conn, id)?;
    if evaluation.situacao == "ANALISADO_BASE_LOCAL" {
        let [critical, high, medium, low] = evaluation.alertas_por_severidade;
        let score = auditor::calcular_score_integridade(critical, high, medium, low, &[]);
        evaluation.score = Some(score.score);
        evaluation.nivel_risco = score.nivel_risco;
        evaluation.cor_hex = score.cor_hex;
    }
    Ok(evaluation)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dados_insuficientes_e_despesa_alta_nao_inventam_risco() {
        let pool = storage::DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        storage::run_migrations(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO politicos(nome_completo,nome_urna) VALUES('MARIA','MARIA')",
            [],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        assert_eq!(avaliar(&conn, id).unwrap().score, None);
        conn.execute("INSERT INTO despesas_parlamentares(casa_legislativa,parlamentar_nome,data_emissao,categoria_despesa,fornecedor_nome,fornecedor_cnpj_cpf,valor_liquido) VALUES('CAMARA','MARIA','2024-01-01','SERVICOS','EMPRESA','11222333000181',400000)",[]).unwrap();
        assert_eq!(avaliar(&conn, id).unwrap().situacao, "NAO_AUDITADO");
        conn.execute(
            "UPDATE auditoria_estado SET pendente=0,concluida_em=CURRENT_TIMESTAMP",
            [],
        )
        .unwrap();
        assert_eq!(avaliar(&conn, id).unwrap().score, Some(100));
        conn.execute("INSERT INTO alertas_auditoria(tipo,severidade,titulo,descricao,alvo_nome,fonte_dado,detalhes_json) VALUES('TESTE','ALTA','Indício','Conferir','MARIA','TESTE',?1)",[format!("{{\"politico_id\":{id}}}")]).unwrap();
        assert_eq!(avaliar(&conn, id).unwrap().score, Some(85));
        crate::config::salvar_parametros_auditoria(
            &conn,
            &crate::config::ParametrosAuditoria::default(),
        )
        .unwrap();
        assert_eq!(avaliar(&conn, id).unwrap().score, None);
    }
}

#[cfg(test)]
mod api_regressions {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    #[tokio::test]
    async fn listagem_e_detalhe_compartilham_avaliacao_sem_penalizar_volume_de_despesas() {
        let pool = storage::DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        storage::run_migrations(&mut conn).unwrap();
        conn.execute_batch("INSERT INTO politicos(id,nome_completo,nome_urna) VALUES(1,'MARIA','MARIA'); INSERT INTO candidaturas(politico_id,ano_eleicao,cargo,sigla_partido,uf) VALUES(1,2024,'PREFEITO','ABC','SP'); INSERT INTO despesas_parlamentares(casa_legislativa,parlamentar_nome,data_emissao,categoria_despesa,fornecedor_nome,fornecedor_cnpj_cpf,valor_liquido) VALUES('CAMARA','MARIA','2024-01-01','SERVICOS','EMPRESA','11222333000181',400000);").unwrap();
        let app = crate::criar_router_com_auth(pool.clone(), crate::security::AdminAuth::default());
        for expected in [None, Some(85)] {
            if expected.is_some() {
                conn.execute("INSERT INTO alertas_auditoria(tipo,severidade,titulo,descricao,alvo_nome,fonte_dado,detalhes_json) VALUES('TESTE','ALTA','Conferir','Indício','MARIA','TESTE','{\"politico_id\":1}')",[]).unwrap();
                conn.execute(
                    "UPDATE auditoria_estado SET pendente=0,concluida_em=CURRENT_TIMESTAMP",
                    [],
                )
                .unwrap();
            }
            let list = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v1/politicos")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(list.status(), 200);
            let list: crate::ListarPoliticosResponse =
                serde_json::from_slice(&to_bytes(list.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            let detail = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v1/politicos/1")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(detail.status(), 200);
            let detail: crate::PoliticoDetalheResponse =
                serde_json::from_slice(&to_bytes(detail.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            assert_eq!(list.politicos[0].score_integridade, expected);
            assert_eq!(detail.score_integridade, expected);
            assert_eq!(
                list.politicos[0].avaliacao_integridade,
                detail.avaliacao_integridade
            );
        }
    }
}
