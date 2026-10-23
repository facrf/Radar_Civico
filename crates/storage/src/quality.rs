//! Contrato único de avaliação: somente alertas persistidos de uma auditoria atual.
use crate::{
    rusqlite::{params, Connection, OptionalExtension},
    Result,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AvaliacaoPolitico {
    pub situacao: String,
    pub score: Option<u32>,
    pub nivel_risco: String,
    pub cor_hex: String,
    pub ultima_auditoria: Option<String>,
    pub metodologia: String,
    pub cobertura: String,
    pub alertas_por_severidade: [usize; 4],
}

pub fn avaliacao_politico(conn: &Connection, id: i64) -> Result<AvaliacaoPolitico> {
    let mut map = avaliacao_politicos_lote(conn, &[id])?;
    Ok(map.remove(&id).unwrap_or_else(|| AvaliacaoPolitico {
        situacao: "SEM_DADOS".into(),
        score: None,
        nivel_risco: "NÃO AVALIADO".into(),
        cor_hex: "#94a3b8".into(),
        ultima_auditoria: None,
        metodologia: String::new(),
        cobertura: "Somente bases locais importadas; ausência de alertas não comprova integridade.".into(),
        alertas_por_severidade: [0; 4],
    }))
}

/// Avalia múltiplos políticos em lote, reutilizando consultas globais e comandos SQL preparados.
pub fn avaliacao_politicos_lote(
    conn: &Connection,
    ids: &[i64],
) -> Result<std::collections::HashMap<i64, AvaliacaoPolitico>> {
    let mut map = std::collections::HashMap::new();
    if ids.is_empty() {
        return Ok(map);
    }

    let (pendente, concluida_em, metodologia): (bool, Option<String>, String) = conn.query_row(
        "SELECT pendente,concluida_em,metodologia FROM auditoria_estado WHERE id=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    let mut stmt_pessoa = conn.prepare_cached("SELECT nome_completo,nome_urna FROM politicos WHERE id=?1")?;
    let mut stmt_dados = conn.prepare_cached(
        "SELECT EXISTS(SELECT 1 FROM candidaturas WHERE politico_id=?1)
          OR EXISTS(SELECT 1 FROM despesas_parlamentares WHERE parlamentar_id=?1 OR parlamentar_nome IN (?2,?3))
          OR EXISTS(SELECT 1 FROM cargos_autoridades WHERE politico_id=?1 AND evidencia_verificada=1)",
    )?;
    let mut stmt_unico = conn.prepare_cached(
        "SELECT COUNT(*)=1 FROM politicos WHERE nome_completo IN (?1,?2) OR nome_urna IN (?1,?2)",
    )?;
    let mut stmt_alertas = conn.prepare_cached(
        "SELECT UPPER(severidade),COUNT(*) FROM alertas_auditoria
         WHERE (json_valid(detalhes_json) AND json_extract(detalhes_json,'$.politico_id')=?1)
            OR (?4 AND alvo_nome IN (?2,?3)) GROUP BY UPPER(severidade)",
    )?;

    for &id in ids {
        let mut result = AvaliacaoPolitico {
            situacao: "SEM_DADOS".into(),
            score: None,
            nivel_risco: "NÃO AVALIADO".into(),
            cor_hex: "#94a3b8".into(),
            ultima_auditoria: concluida_em.clone(),
            metodologia: metodologia.clone(),
            cobertura: "Somente bases locais importadas; ausência de alertas não comprova integridade.".into(),
            alertas_por_severidade: [0; 4],
        };

        let pessoa: Option<(String, String)> = stmt_pessoa
            .query_row([id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;

        let Some((nome, urna)) = pessoa else {
            map.insert(id, result);
            continue;
        };

        let possui_dados: bool = stmt_dados.query_row(params![id, nome, urna], |r| r.get(0))?;
        if !possui_dados {
            map.insert(id, result);
            continue;
        }

        if concluida_em.is_none() {
            result.situacao = "NAO_AUDITADO".into();
            map.insert(id, result);
            continue;
        }

        if pendente {
            result.situacao = "DESATUALIZADA".into();
            map.insert(id, result);
            continue;
        }

        let nome_unico: bool = stmt_unico.query_row(params![nome, urna], |r| r.get(0))?;

        let rows = stmt_alertas.query_map(params![id, nome, urna, nome_unico], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, usize>(1)?))
        })?;

        for row in rows {
            let (severity, count) = row?;
            match severity.as_str() {
                "CRITICA" | "CRÍTICA" => result.alertas_por_severidade[0] += count,
                "ALTA" | "GRAVE" => result.alertas_por_severidade[1] += count,
                "MEDIA" | "MÉDIA" => result.alertas_por_severidade[2] += count,
                "BAIXA" | "LEVE" => result.alertas_por_severidade[3] += count,
                _ => {}
            }
        }

        result.situacao = "ANALISADO_BASE_LOCAL".into();
        map.insert(id, result);
    }

    Ok(map)
}
