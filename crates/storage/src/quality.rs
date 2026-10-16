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
    let (pendente, concluida_em, metodologia): (bool, Option<String>, String) = conn.query_row(
        "SELECT pendente,concluida_em,metodologia FROM auditoria_estado WHERE id=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let pessoa: Option<(String, String)> = conn
        .query_row(
            "SELECT nome_completo,nome_urna FROM politicos WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let mut result = AvaliacaoPolitico {
        situacao: "SEM_DADOS".into(),
        score: None,
        nivel_risco: "NÃO AVALIADO".into(),
        cor_hex: "#94a3b8".into(),
        ultima_auditoria: concluida_em.clone(),
        metodologia,
        cobertura: "Somente bases locais importadas; ausência de alertas não comprova integridade."
            .into(),
        alertas_por_severidade: [0; 4],
    };
    let Some((nome, urna)) = pessoa else {
        return Ok(result);
    };
    let possui_dados: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM candidaturas WHERE politico_id=?1)
          OR EXISTS(SELECT 1 FROM despesas_parlamentares WHERE parlamentar_id=?1 OR parlamentar_nome IN (?2,?3))
          OR EXISTS(SELECT 1 FROM cargos_autoridades WHERE politico_id=?1 AND evidencia_verificada=1)",
        params![id,nome,urna], |r| r.get(0),
    )?;
    if !possui_dados {
        return Ok(result);
    }
    if concluida_em.is_none() {
        result.situacao = "NAO_AUDITADO".into();
        return Ok(result);
    }
    if pendente {
        result.situacao = "DESATUALIZADA".into();
        return Ok(result);
    }
    // O nome só associa alertas se resolver para uma única identidade no cadastro.
    let nome_unico: bool = conn.query_row(
        "SELECT COUNT(*)=1 FROM politicos WHERE nome_completo IN (?1,?2) OR nome_urna IN (?1,?2)",
        params![nome, urna],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT UPPER(severidade),COUNT(*) FROM alertas_auditoria
         WHERE (json_valid(detalhes_json) AND json_extract(detalhes_json,'$.politico_id')=?1)
            OR (?4 AND alvo_nome IN (?2,?3)) GROUP BY UPPER(severidade)",
    )?;
    let rows = stmt.query_map(params![id, nome, urna, nome_unico], |r| {
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
    Ok(result)
}
