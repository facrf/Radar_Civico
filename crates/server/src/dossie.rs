use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use storage::rusqlite;
use storage::DbPool;

// ============================================================================
// Tipos e Estruturas para Dossiê Analítico
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaDossie {
    pub tipo: String,
    pub severidade: String, // "INFO" | "BAIXA" | "MEDIA" | "ALTA" | "CRITICA"
    pub titulo: String,
    pub descricao: String,
    pub valor_envolvido: Option<f64>,
    pub fonte: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmpresaResumo {
    pub cnpj: String,
    pub cnpj_formatado: String,
    pub razao_social: String,
    pub vinculo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SocioItem {
    pub nome: String,
    pub documento_mascarado: String,
    pub qualificacao: Option<String>,
    pub tipo: String, // "PF" | "PJ"
    pub outras_empresas: Vec<EmpresaResumo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PainelSocietario {
    pub total_socios: usize,
    pub socios: Vec<SocioItem>,
    pub empresas_interligadas: Vec<EmpresaResumo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotaFiscalCeapItem {
    pub id: i64,
    pub parlamentar_nome: String,
    pub data_emissao: String,
    pub categoria_despesa: String,
    pub valor_liquido: f64,
    pub numero_documento: Option<String>,
    pub url_nota_fiscal: Option<String>,
    pub flag_anomalia: bool,
    #[serde(default)]
    pub volume_estimado: bool,
    #[serde(default)]
    pub volume_litros: Option<f64>,
    pub politico_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompradorCeapResumo {
    pub parlamentar_nome: String,
    pub total_gasto: f64,
    pub quantidade_notas: usize,
    pub politico_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PainelCeap {
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub limit: usize,
    pub total_faturado: f64,
    pub total_notas: usize,
    pub compradores: Vec<CompradorCeapResumo>,
    pub notas_fiscais: Vec<NotaFiscalCeapItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContratoPncpItem {
    pub id: i64,
    pub orgao_contratante: String,
    pub valor_contratado: f64,
    pub objeto: Option<String>,
    pub data_assinatura: Option<String>,
    pub data_termino: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrgaoContratanteResumo {
    pub orgao: String,
    pub total_valor: f64,
    pub quantidade_contratos: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PainelPncp {
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub limit: usize,
    pub total_contratado: f64,
    pub total_contratos: usize,
    pub orgaos_contratantes: Vec<OrgaoContratanteResumo>,
    pub contratos: Vec<ContratoPncpItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DoacaoSocioTse {
    pub socio_nome: String,
    pub doador_doc: String,
    pub candidato_nome: String,
    pub politico_id: Option<i64>,
    pub cargo: String,
    pub partido: String,
    pub ano: i32,
    pub valor: f64,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidaturaSocioTse {
    pub socio_nome: String,
    pub politico_id: i64,
    pub nome_urna: String,
    pub cargo: String,
    pub partido: String,
    pub ano: i32,
    pub uf: String,
    pub total_bens: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PainelTseSocios {
    pub total_doacoes_socios: f64,
    pub doacoes: Vec<DoacaoSocioTse>,
    pub candidaturas_socios: Vec<CandidaturaSocioTse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiarioItem {
    pub termo: String,
    pub data: Option<String>,
    pub ocorrencias: usize,
    pub resumo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DossieCnpjResponse {
    pub cnpj: String,
    pub cnpj_formatado: String,
    pub razao_social: String,
    pub situacao_cadastral: String,
    pub data_abertura: Option<String>,
    pub score_risco: String, // "BAIXO" | "MEDIO" | "ALTO" | "CRITICO"
    pub total_alertas: usize,
    pub alertas: Vec<AlertaDossie>,
    pub qsa: PainelSocietario,
    pub ceap: PainelCeap,
    pub pncp: PainelPncp,
    pub tse: PainelTseSocios,
    pub diarios: Vec<DiarioItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmpresaSocioItem {
    pub cnpj: String,
    pub cnpj_formatado: String,
    pub razao_social: String,
    pub qualificacao: Option<String>,
    pub total_ceap: f64,
    pub total_pncp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DoacaoEleitoralItem {
    pub id: i64,
    pub doador_cpf_cnpj: String,
    pub doador_nome: String,
    pub valor: f64,
    pub data_receita: Option<String>,
    pub ano_eleicao: i32,
    pub cargo: String,
    pub partido: String,
    pub politico_nome: String,
    pub politico_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidaturaItemCpf {
    pub politico_id: i64,
    pub nome_urna: String,
    pub ano_eleicao: i32,
    pub cargo: String,
    pub partido: String,
    pub uf: String,
    pub municipio: Option<String>,
    pub total_bens: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BeneficioEmergencialItem {
    pub id: i64,
    pub mes: String,
    pub parcela: Option<String>,
    pub valor: f64,
    pub enquadramento: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistroProfissionalItem {
    pub orgao: String,
    pub numero: String,
    pub uf: String,
    pub situacao: String,
    pub tipo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DossieCpfResponse {
    pub cpf_mascarado: String,
    pub nome: String,
    pub score_risco: String, // "BAIXO" | "MEDIO" | "ALTO" | "CRITICO"
    pub total_alertas: usize,
    pub alertas: Vec<AlertaDossie>,
    pub empresas_socio: Vec<EmpresaSocioItem>,
    pub total_faturado_empresas_ceap: f64,
    pub total_contratado_empresas_pncp: f64,
    pub doacoes_eleitorais: Vec<DoacaoEleitoralItem>,
    pub candidaturas: Vec<CandidaturaItemCpf>,
    pub beneficios_emergenciais: Vec<BeneficioEmergencialItem>,
    pub registros_profissionais: Vec<RegistroProfissionalItem>,
    pub notas_ceap_empresas: Vec<NotaFiscalCeapItem>,
    pub contratos_pncp_empresas: Vec<ContratoPncpItem>,
}

// ============================================================================
// Utilitários de Limpeza e Formatação
// ============================================================================

pub fn limpar_documento(doc: &str) -> String {
    doc.chars().filter(|c| c.is_ascii_digit()).collect()
}

pub fn formatar_cnpj(digits: &str) -> String {
    if digits.len() == 14 {
        format!(
            "{}.{}.{}/{}-{}",
            &digits[0..2],
            &digits[2..5],
            &digits[5..8],
            &digits[8..12],
            &digits[12..14]
        )
    } else if digits.len() == 8 {
        format!("{}.{}.{}", &digits[0..2], &digits[2..5], &digits[5..8])
    } else {
        digits.to_string()
    }
}

pub fn formatar_cpf_mascarado(digits_central: &str) -> String {
    if digits_central.len() == 6 {
        format!("***.{}.{}-**", &digits_central[0..3], &digits_central[3..6])
    } else {
        digits_central.to_string()
    }
}

pub fn extrair_miolo_cpf(s: &str) -> Option<String> {
    let digits = limpar_documento(s);
    if digits.len() == 11 {
        Some(digits[3..9].to_string())
    } else if digits.len() == 6 {
        Some(digits)
    } else {
        None
    }
}

pub fn normalizar_nome(s: &str) -> String {
    s.trim()
        .to_uppercase()
        .chars()
        .map(|c| match c {
            'Á' | 'À' | 'Ã' | 'Â' | 'Ä' => 'A',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'Ó' | 'Ò' | 'Õ' | 'Ô' | 'Ö' => 'O',
            'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'Ç' => 'C',
            _ => c,
        })
        .collect()
}

fn calcular_score_risco(alertas: &[AlertaDossie]) -> String {
    if alertas.iter().any(|a| a.severidade == "CRITICA") {
        "CRITICO".to_string()
    } else if alertas.iter().any(|a| a.severidade == "ALTA") {
        "ALTO".to_string()
    } else if alertas.iter().any(|a| a.severidade == "MEDIA") {
        "MEDIO".to_string()
    } else {
        "BAIXO".to_string()
    }
}

// ============================================================================
// Endpoint: Dossiê Completo de CNPJ
// ============================================================================

#[derive(Debug, Default, Deserialize)]
pub struct DossiePagination {
    pub ceap_offset: Option<usize>,
    pub ceap_limit: Option<usize>,
    pub pncp_offset: Option<usize>,
    pub pncp_limit: Option<usize>,
}
fn database_error(error: impl std::fmt::Display) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"erro":error.to_string()})),
    )
}
/// Agregados completos em centavos; limites das listas não alteram o total.
fn supplier_totals(
    conn: &rusqlite::Connection,
    root: &str,
) -> storage::Result<(f64, usize, f64, usize)> {
    let pattern = format!("%{}%", root);
    let (ceap,nceap):(i64,usize)=conn.query_row("SELECT COALESCE(SUM(valor_liquido_centavos),0),COUNT(*) FROM despesas_parlamentares WHERE fornecedor_cnpj_cpf LIKE ?1",[&pattern],|r|Ok((r.get(0)?,r.get(1)?)))?;
    let (pncp,npncp):(i64,usize)=conn.query_row("SELECT COALESCE(SUM(valor_contratado_centavos),0),COUNT(*) FROM contratos_publicos WHERE fornecedor_cnpj LIKE ?1",[&pattern],|r|Ok((r.get(0)?,r.get(1)?)))?;
    Ok((
        storage::Money::from_cents(ceap).reais(),
        nceap,
        storage::Money::from_cents(pncp).reais(),
        npncp,
    ))
}

pub async fn dossie_cnpj_handler(
    Path(cnpj_param): Path<String>,
    State(pool): State<DbPool>,
    Query(pagination): Query<DossiePagination>,
) -> Result<Json<DossieCnpjResponse>, (StatusCode, Json<serde_json::Value>)> {
    let conn = pool.get().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro": format!("Erro ao obter conexão: {e}")})),
        )
    })?;

    let digits = limpar_documento(&cnpj_param);
    let cnpj_basico = if digits.len() >= 8 {
        &digits[0..8]
    } else {
        &digits
    };

    if cnpj_basico.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"erro": "CNPJ informado inválido ou vazio"})),
        ));
    }

    // 1. Consulta em empresas_qsa
    let mut socios: Vec<SocioItem> = Vec::new();
    let mut razao_social = String::new();
    let mut cnpj_ordem = "0001".to_string();
    let mut cnpj_dv = "00".to_string();

    {
        let mut stmt = conn
            .prepare(
                "SELECT cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_cpf_cnpj_mascarado, socio_nome, qualificacao_socio
                 FROM empresas_qsa
                 WHERE cnpj_basico = ?1",
            )
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"erro": format!("Erro SQL QSA: {e}")})),
                )
            })?;

        let rows = stmt
            .query_map([cnpj_basico], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"erro": format!("Erro query QSA: {e}")})),
                )
            })?;

        for r in rows.flatten() {
            if razao_social.is_empty() {
                razao_social = r.3.clone();
                cnpj_ordem = r.1.clone();
                cnpj_dv = r.2.clone();
            }

            let s_doc = r.4;
            let s_nome = r.5;
            let qualif = r.6;
            let tipo = if s_doc.contains('/') || limpar_documento(&s_doc).len() == 14 {
                "PJ".to_string()
            } else {
                "PF".to_string()
            };

            socios.push(SocioItem {
                nome: if s_nome.is_empty() {
                    s_doc.clone()
                } else {
                    s_nome
                },
                documento_mascarado: s_doc,
                qualificacao: qualif,
                tipo,
                outras_empresas: Vec::new(),
            });
        }
    }

    // Se não encontrou no QSA, busca razão social/nome nas despesas parlamentares ou contratos
    if razao_social.is_empty() {
        let like_cnpj = format!("%{}%", cnpj_basico);
        let nome_ceap: Option<String> = conn
            .query_row(
                "SELECT fornecedor_nome FROM despesas_parlamentares WHERE fornecedor_cnpj_cpf LIKE ?1 LIMIT 1",
                [&like_cnpj],
                |r| r.get(0),
            )
            .ok();

        if let Some(n) = nome_ceap {
            razao_social = n;
        } else {
            let nome_pncp: Option<String> = conn
                .query_row(
                    "SELECT fornecedor_cnpj FROM contratos_publicos WHERE fornecedor_cnpj LIKE ?1 LIMIT 1",
                    [&like_cnpj],
                    |r| r.get(0),
                )
                .ok();
            razao_social = nome_pncp.unwrap_or_else(|| format!("Empresa CNPJ {}", cnpj_basico));
        }
    }

    let cnpj_completo_digits = format!("{}{}{}", cnpj_basico, cnpj_ordem, cnpj_dv);
    let cnpj_formatado = formatar_cnpj(&cnpj_completo_digits);

    // 2. Busca de outras empresas interligadas aos sócios
    let mut empresas_interligadas: Vec<EmpresaResumo> = Vec::new();
    for socio in &mut socios {
        if socio.documento_mascarado.is_empty() || socio.documento_mascarado == "***.***.***-**" {
            continue;
        }

        let stmt_outras = conn
            .prepare(
                "SELECT DISTINCT cnpj_basico, cnpj_ordem, cnpj_dv, razao_social
                 FROM empresas_qsa
                 WHERE socio_cpf_cnpj_mascarado = ?1 AND cnpj_basico != ?2
                 LIMIT 8",
            )
            .ok();

        if let Some(mut stmt) = stmt_outras {
            let rows = stmt
                .query_map(
                    [&socio.documento_mascarado, &cnpj_basico.to_string()],
                    |row| {
                        let b: String = row.get(0)?;
                        let o: String = row.get(1)?;
                        let d: String = row.get(2)?;
                        let r: String = row.get(3)?;
                        let c_full = format!("{}{}{}", b, o, d);
                        Ok(EmpresaResumo {
                            cnpj: c_full.clone(),
                            cnpj_formatado: formatar_cnpj(&c_full),
                            razao_social: r,
                            vinculo: format!("Sócio comum: {}", socio.nome),
                        })
                    },
                )
                .ok();

            if let Some(r_iter) = rows {
                for emp in r_iter.flatten() {
                    if !empresas_interligadas.iter().any(|e| e.cnpj == emp.cnpj) {
                        empresas_interligadas.push(emp.clone());
                    }
                    socio.outras_empresas.push(emp);
                }
            }
        }
    }

    let params_auditoria = crate::routes::config::carregar_parametros_auditoria(&conn);

    // 3. Gastos Parlamentares (CEAP)
    let mut notas_fiscais: Vec<NotaFiscalCeapItem> = Vec::new();
    let ceap_limit = pagination.ceap_limit.unwrap_or(150).clamp(1, 150);
    let ceap_offset = pagination.ceap_offset.unwrap_or(0).min(i64::MAX as usize);
    let pncp_limit = pagination.pncp_limit.unwrap_or(50).clamp(1, 50);
    let pncp_offset = pagination.pncp_offset.unwrap_or(0).min(i64::MAX as usize);
    let (total_faturado_ceap, total_notas_ceap, total_contratado_pncp, total_contratos_pncp) =
        supplier_totals(&conn, &cnpj_basico).map_err(database_error)?;
    let mut compradores_map: std::collections::HashMap<String, (f64, usize)> =
        std::collections::HashMap::new();
    let mut stmt=conn.prepare("SELECT parlamentar_nome,COALESCE(SUM(valor_liquido_centavos),0),COUNT(*) FROM despesas_parlamentares WHERE fornecedor_cnpj_cpf LIKE ?1 GROUP BY parlamentar_nome").map_err(database_error)?;
    let rows = stmt
        .query_map([format!("%{}%", cnpj_basico)], |r| {
            Ok((
                r.get::<_, String>(0)?,
                storage::Money::from_cents(r.get(1)?).reais(),
                r.get::<_, usize>(2)?,
            ))
        })
        .map_err(database_error)?;
    for row in rows {
        let (name, total, count) = row.map_err(database_error)?;
        compradores_map.insert(name, (total, count));
    }

    {
        let like_basico = format!("%{}%", cnpj_basico);
        let stmt_ceap = conn
            .prepare(
                "SELECT dp.id, dp.parlamentar_nome, dp.data_emissao, dp.categoria_despesa,
                        dp.valor_liquido, dp.numero_documento, dp.url_nota_fiscal, dp.flag_anomalia,
                        (SELECT p.id FROM politicos p WHERE p.nome_completo = dp.parlamentar_nome OR p.nome_urna = dp.parlamentar_nome LIMIT 1) as politico_id,
                        dp.detalhes_litros
                 FROM despesas_parlamentares dp
                 WHERE dp.fornecedor_cnpj_cpf LIKE ?1
                 ORDER BY dp.data_emissao DESC, dp.id DESC
                 LIMIT ?2 OFFSET ?3",
            )
            .ok();

        if let Some(mut stmt) = stmt_ceap {
            let rows = stmt
                .query_map(
                    rusqlite::params![like_basico, ceap_limit as i64, ceap_offset as i64],
                    |row| {
                        let cat: String = row.get(3)?;
                        let val: f64 = row.get(4)?;
                        let litros_opt: Option<f64> = row.get(9)?;
                        let volume = params_auditoria.volume_combustivel(&cat, val, litros_opt);
                        let volume_estimado = volume.is_some_and(|(_, estimated)| estimated);
                        let volume_litros = volume.map(|(liters, _)| liters);
                        let flag_anomalia = volume_litros.is_some_and(|liters| {
                            liters > params_auditoria.limite_combustivel_litros
                        });

                        Ok(NotaFiscalCeapItem {
                            id: row.get(0)?,
                            parlamentar_nome: row.get(1)?,
                            data_emissao: row.get(2)?,
                            categoria_despesa: cat,
                            valor_liquido: val,
                            numero_documento: row.get(5)?,
                            url_nota_fiscal: row.get(6)?,
                            flag_anomalia,
                            volume_estimado,
                            volume_litros,
                            politico_id: row.get(8)?,
                        })
                    },
                )
                .ok();

            if let Some(r_iter) = rows {
                for nf in r_iter.flatten() {
                    notas_fiscais.push(nf);
                }
            }
        }
    }

    let mut compradores: Vec<CompradorCeapResumo> = compradores_map
        .into_iter()
        .map(|(nome, (val, qtd))| {
            let p_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM politicos WHERE nome_completo = ?1 OR nome_urna = ?1 LIMIT 1",
                    [&nome],
                    |r| r.get(0),
                )
                .ok();

            CompradorCeapResumo {
                parlamentar_nome: nome,
                total_gasto: val,
                quantidade_notas: qtd,
                politico_id: p_id,
            }
        })
        .collect();
    compradores.sort_by(|a, b| {
        b.total_gasto
            .partial_cmp(&a.total_gasto)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 4. Contratos Públicos (PNCP)
    let mut contratos: Vec<ContratoPncpItem> = Vec::new();

    let mut orgaos_map: std::collections::HashMap<String, (f64, usize)> =
        std::collections::HashMap::new();

    let mut stmt=conn.prepare("SELECT orgao_contratante,COALESCE(SUM(valor_contratado_centavos),0),COUNT(*) FROM contratos_publicos WHERE fornecedor_cnpj LIKE ?1 GROUP BY orgao_contratante").map_err(database_error)?;
    let rows = stmt
        .query_map([format!("%{}%", cnpj_basico)], |r| {
            Ok((
                r.get::<_, String>(0)?,
                storage::Money::from_cents(r.get(1)?).reais(),
                r.get::<_, usize>(2)?,
            ))
        })
        .map_err(database_error)?;
    for row in rows {
        let (name, total, count) = row.map_err(database_error)?;
        orgaos_map.insert(name, (total, count));
    }
    {
        let like_basico = format!("%{}%", cnpj_basico);
        let stmt_pncp = conn
            .prepare(
                "SELECT id, orgao_contratante, valor_contratado, objeto, data_assinatura, data_termino
                 FROM contratos_publicos
                 WHERE fornecedor_cnpj LIKE ?1
                 ORDER BY data_assinatura DESC, id DESC
                 LIMIT ?2 OFFSET ?3",
            )
            .ok();

        if let Some(mut stmt) = stmt_pncp {
            let rows = stmt
                .query_map(
                    rusqlite::params![like_basico, pncp_limit as i64, pncp_offset as i64],
                    |row| {
                        Ok(ContratoPncpItem {
                            id: row.get(0)?,
                            orgao_contratante: row.get(1)?,
                            valor_contratado: row.get(2)?,
                            objeto: row.get(3)?,
                            data_assinatura: row.get(4)?,
                            data_termino: row.get(5)?,
                        })
                    },
                )
                .ok();

            if let Some(r_iter) = rows {
                for c in r_iter.flatten() {
                    contratos.push(c);
                }
            }
        }
    }

    let mut orgaos_contratantes: Vec<OrgaoContratanteResumo> = orgaos_map
        .into_iter()
        .map(|(orgao, (val, qtd))| OrgaoContratanteResumo {
            orgao,
            total_valor: val,
            quantidade_contratos: qtd,
        })
        .collect();
    orgaos_contratantes.sort_by(|a, b| {
        b.total_valor
            .partial_cmp(&a.total_valor)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 5. Cruzamento Heurístico TSE para cada Sócio PF (Nome normalizado + 6 dígitos centrais do CPF)
    let mut doacoes_socios: Vec<DoacaoSocioTse> = Vec::new();
    let mut candidaturas_socios: Vec<CandidaturaSocioTse> = Vec::new();
    let mut donation_conditions = Vec::new();
    let mut donation_params = Vec::new();

    for socio in &socios {
        if socio.tipo != "PF" {
            continue;
        }

        let miolo_opt = extrair_miolo_cpf(&socio.documento_mascarado);
        let nome_norm = normalizar_nome(&socio.nome);

        // Doações Eleitorais do Sócio
        let mut sql_doacoes = String::from(
            "SELECT rc.doador_cpf_cnpj, rc.valor, rc.data_receita,
                    c.ano_eleicao, c.cargo, c.sigla_partido, p.nome_completo, p.id
             FROM receitas_campanha rc
             JOIN candidaturas c ON rc.candidatura_id = c.id
             JOIN politicos p ON c.politico_id = p.id
             WHERE 1=0 ",
        );

        let mut params_doacoes: Vec<String> = Vec::new();
        if let Some(ref miolo) = miolo_opt {
            sql_doacoes.push_str("OR rc.doador_cpf_cnpj LIKE ? ");
            params_doacoes.push(format!("%{}%", miolo));
            donation_conditions.push("rc.doador_cpf_cnpj LIKE ?");
            donation_params.push(format!("%{}%", miolo));
        }
        if !nome_norm.is_empty() && nome_norm.len() >= 5 {
            sql_doacoes.push_str("OR rc.doador_nome LIKE ? ");
            params_doacoes.push(format!("%{}%", nome_norm));
            donation_conditions.push("rc.doador_nome LIKE ?");
            donation_params.push(format!("%{}%", nome_norm));
        }
        sql_doacoes.push_str("LIMIT 20");

        if !params_doacoes.is_empty() {
            if let Ok(mut stmt) = conn.prepare(&sql_doacoes) {
                let rusqlite_params = rusqlite::params_from_iter(params_doacoes.iter());
                if let Ok(rows) = stmt.query_map(rusqlite_params, |r| {
                    Ok(DoacaoSocioTse {
                        socio_nome: socio.nome.clone(),
                        doador_doc: r.get(0)?,
                        valor: r.get(1)?,
                        data: r.get(2)?,
                        ano: r.get(3)?,
                        cargo: r.get(4)?,
                        partido: r.get(5)?,
                        candidato_nome: r.get(6)?,
                        politico_id: r.get(7)?,
                    })
                }) {
                    for d in rows.flatten() {
                        doacoes_socios.push(d);
                    }
                }
            }
        }

        // Candidaturas do Sócio
        let mut sql_cand = String::from(
            "SELECT p.id, p.nome_urna, c.cargo, c.sigla_partido, c.ano_eleicao, c.uf, c.total_bens_declarados
             FROM politicos p
             JOIN candidaturas c ON c.politico_id = p.id
             WHERE 1=0 ",
        );

        let mut params_cand: Vec<String> = Vec::new();
        if let Some(ref miolo) = miolo_opt {
            sql_cand.push_str("OR p.cpf_mascarado LIKE ? ");
            params_cand.push(format!("%{}%", miolo));
        }
        if !nome_norm.is_empty() && nome_norm.len() >= 5 {
            sql_cand.push_str("OR p.nome_completo LIKE ? ");
            params_cand.push(format!("%{}%", nome_norm));
        }
        sql_cand.push_str("LIMIT 10");

        if !params_cand.is_empty() {
            if let Ok(mut stmt) = conn.prepare(&sql_cand) {
                let rusqlite_params = rusqlite::params_from_iter(params_cand.iter());
                if let Ok(rows) = stmt.query_map(rusqlite_params, |r| {
                    Ok(CandidaturaSocioTse {
                        socio_nome: socio.nome.clone(),
                        politico_id: r.get(0)?,
                        nome_urna: r.get(1)?,
                        cargo: r.get(2)?,
                        partido: r.get(3)?,
                        ano: r.get(4)?,
                        uf: r.get(5)?,
                        total_bens: r.get(6)?,
                    })
                }) {
                    for c in rows.flatten() {
                        candidaturas_socios.push(c);
                    }
                }
            }
        }
    }

    // 6. Consultas a Diários Oficiais
    let mut diarios: Vec<DiarioItem> = Vec::new();
    {
        let like_cnpj = format!("%{}%", cnpj_basico);
        let stmt_d = conn
            .prepare(
                "SELECT termo_pesquisado, ocorrencias_encontradas, data_consulta
                 FROM cache_consultas_diario
                 WHERE doador_cpf_cnpj LIKE ?1 OR termo_pesquisado LIKE ?2
                 LIMIT 5",
            )
            .ok();

        if let Some(mut stmt) = stmt_d {
            let rows = stmt
                .query_map([&like_cnpj, &razao_social], |row| {
                    Ok(DiarioItem {
                        termo: row.get(0)?,
                        ocorrencias: row.get::<_, i64>(1).unwrap_or(0) as usize,
                        data: row.get(2)?,
                        resumo: None,
                    })
                })
                .ok();

            if let Some(r_iter) = rows {
                for d in r_iter.flatten() {
                    diarios.push(d);
                }
            }
        }
    }

    // 7. Detecção Heurística de Alertas e Anomalias
    let mut alertas: Vec<AlertaDossie> = Vec::new();

    // REGRA 1: Triangulação Eleitoral (Sócio doou para político que comprou da empresa via CEAP)
    for comprador in &compradores {
        let comprador_norm = normalizar_nome(&comprador.parlamentar_nome);
        for doacao in &doacoes_socios {
            let candidato_norm = normalizar_nome(&doacao.candidato_nome);
            if comprador_norm == candidato_norm
                || (comprador_norm.len() >= 8 && candidato_norm.contains(&comprador_norm))
                || (candidato_norm.len() >= 8 && comprador_norm.contains(&candidato_norm))
            {
                alertas.push(AlertaDossie {
                    tipo: "TRIANGULACAO_HUB".to_string(),
                    severidade: "CRITICA".to_string(),
                    titulo: "Triangulação Eleitoral / Fornecedor CEAP".to_string(),
                    descricao: format!(
                        "O sócio {} realizou doação de campanha de R$ {:.2} ({}) para {}, que contratou esta empresa na Câmara/CEAP com R$ {:.2} faturados.",
                        doacao.socio_nome, doacao.valor, doacao.partido, comprador.parlamentar_nome, comprador.total_gasto
                    ),
                    valor_envolvido: Some(doacao.valor + comprador.total_gasto),
                    fonte: "Cruzamento CEAP + TSE Prestação de Contas".to_string(),
                });
            }
        }
    }

    // REGRA 2: Fornecedor Hub / Concentração de Fornecedor por Parlamentar
    let mut max_concentracao = 0.0;
    let mut max_comprador_nome = String::new();
    let mut max_comprador_gasto = 0.0;
    if total_faturado_ceap > 0.0 {
        for c in &compradores {
            let pct = (c.total_gasto / total_faturado_ceap) * 100.0;
            if pct > max_concentracao {
                max_concentracao = pct;
                max_comprador_nome = c.parlamentar_nome.clone();
                max_comprador_gasto = c.total_gasto;
            }
        }
    }

    if max_concentracao >= params_auditoria.concentracao_fornecedor_percentual
        && total_faturado_ceap > 10_000.0
    {
        alertas.push(AlertaDossie {
            tipo: "FORNECEDOR_HUB".to_string(),
            severidade: "ALTA".to_string(),
            titulo: "Alta Concentração de Fornecedor CEAP".to_string(),
            descricao: format!(
                "O parlamentar {} concentrou {:.1}% do faturamento desta empresa na CEAP (limiar configurado: {:.0}%). Total pago: R$ {:.2} de R$ {:.2}.",
                max_comprador_nome, max_concentracao, params_auditoria.concentracao_fornecedor_percentual, max_comprador_gasto, total_faturado_ceap
            ),
            valor_envolvido: Some(max_comprador_gasto),
            fonte: "Câmara dos Deputados (CEAP)".to_string(),
        });
    } else if total_faturado_ceap > 500_000.0 || compradores.len() >= 5 {
        alertas.push(AlertaDossie {
            tipo: "FORNECEDOR_HUB".to_string(),
            severidade: "ALTA".to_string(),
            titulo: "Fornecedor Hub de Cotas Parlamentares".to_string(),
            descricao: format!(
                "Empresa concentrou R$ {:.2} em despesas faturadas para {} gabinetes parlamentares distintos.",
                total_faturado_ceap,
                compradores.len()
            ),
            valor_envolvido: Some(total_faturado_ceap),
            fonte: "Câmara dos Deputados (CEAP)".to_string(),
        });
    }

    // REGRA 3: Notas com Anomalia de Combustível (Volume > Limite Configurado)
    let notas_anomalas = notas_fiscais.iter().filter(|n| n.flag_anomalia).count();
    if notas_anomalas > 0 {
        alertas.push(AlertaDossie {
            tipo: "COMBUSTIVEL_VOLUME".to_string(),
            severidade: "MEDIA".to_string(),
            titulo: "Indícios de Volume Elevado em Combustível".to_string(),
            descricao: format!(
                "{} nota(s) ultrapassam o limite configurado de {:.0} L; {} usam volume estimado pelo preço de referência informado. Conferir a nota e sua abrangência antes de concluir irregularidade.",
                notas_anomalas, params_auditoria.limite_combustivel_litros,
                notas_fiscais.iter().filter(|n| n.flag_anomalia && n.volume_estimado).count()
            ),
            valor_envolvido: None,
            fonte: "Auditor CEAP Determinístico".to_string(),
        });
    }

    // REGRA 4: Contratos Vultosos no PNCP
    if total_contratado_pncp > 1_000_000.0 {
        alertas.push(AlertaDossie {
            tipo: "CONTRATOS_EXPRESSIVOS".to_string(),
            severidade: "INFO".to_string(),
            titulo: "Volume Expressivo em Contratações Públicas".to_string(),
            descricao: format!(
                "Empresa possui R$ {:.2} em {} contratos públicos catalogados no PNCP.",
                total_contratado_pncp,
                contratos.len()
            ),
            valor_envolvido: Some(total_contratado_pncp),
            fonte: "Portal Nacional de Contratações Públicas (PNCP)".to_string(),
        });
    }

    // Alertas Gerais pré-registrados na base
    {
        let like_cnpj = format!("%{}%", cnpj_basico);
        let stmt_al = conn
            .prepare(
                "SELECT tipo, severidade, titulo, descricao, valor_envolvido, fonte_dado
                 FROM alertas_auditoria
                 WHERE alvo_documento LIKE ?1 OR alvo_nome LIKE ?2
                 LIMIT 5",
            )
            .ok();

        if let Some(mut stmt) = stmt_al {
            let rows = stmt
                .query_map([&like_cnpj, &razao_social], |r| {
                    Ok(AlertaDossie {
                        tipo: r.get(0)?,
                        severidade: r.get(1)?,
                        titulo: r.get(2)?,
                        descricao: r.get(3)?,
                        valor_envolvido: r.get(4)?,
                        fonte: r.get(5)?,
                    })
                })
                .ok();

            if let Some(r_iter) = rows {
                for a in r_iter.flatten() {
                    if !alertas.iter().any(|ex| ex.titulo == a.titulo) {
                        alertas.push(a);
                    }
                }
            }
        }
    }

    let score_risco = calcular_score_risco(&alertas);
    let total_doacoes_socios = if donation_conditions.is_empty() {
        0.0
    } else {
        let sql=format!("SELECT COALESCE(SUM(rc.valor_centavos),0) FROM receitas_campanha rc JOIN candidaturas c ON c.id=rc.candidatura_id JOIN politicos p ON p.id=c.politico_id WHERE {}",donation_conditions.join(" OR "));
        let cents: i64 = conn
            .query_row(
                &sql,
                rusqlite::params_from_iter(donation_params.iter()),
                |r| r.get(0),
            )
            .map_err(database_error)?;
        storage::Money::from_cents(cents).reais()
    };

    Ok(Json(DossieCnpjResponse {
        cnpj: cnpj_completo_digits,
        cnpj_formatado,
        razao_social,
        situacao_cadastral: "Ativa (Receita Federal)".to_string(),
        data_abertura: None,
        total_alertas: alertas.len(),
        score_risco,
        alertas,
        qsa: PainelSocietario {
            total_socios: socios.len(),
            socios,
            empresas_interligadas,
        },
        ceap: PainelCeap {
            offset: ceap_offset,
            limit: ceap_limit,
            total_faturado: total_faturado_ceap,
            total_notas: total_notas_ceap,
            compradores,
            notas_fiscais,
        },
        pncp: PainelPncp {
            offset: pncp_offset,
            limit: pncp_limit,
            total_contratado: total_contratado_pncp,
            total_contratos: total_contratos_pncp,
            orgaos_contratantes,
            contratos,
        },
        tse: PainelTseSocios {
            total_doacoes_socios,
            doacoes: doacoes_socios,
            candidaturas_socios,
        },
        diarios,
    }))
}

// ============================================================================
// Endpoint: Dossiê Completo de CPF / Pessoa Física / Sócio
// ============================================================================

pub async fn dossie_cpf_handler(
    Path(cpf_param): Path<String>,
    State(pool): State<DbPool>,
) -> Result<Json<DossieCpfResponse>, (StatusCode, Json<serde_json::Value>)> {
    let conn = pool.get().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"erro": format!("Erro ao obter conexão: {e}")})),
        )
    })?;

    let params_auditoria = crate::routes::config::carregar_parametros_auditoria(&conn);

    let digits = limpar_documento(&cpf_param);
    let miolo_opt = extrair_miolo_cpf(&cpf_param);
    let termo_nome = if digits.len() < 6 {
        cpf_param.trim().to_uppercase()
    } else {
        String::new()
    };

    let cpf_formatado = if let Some(ref m) = miolo_opt {
        formatar_cpf_mascarado(m)
    } else if !digits.is_empty() {
        digits.clone()
    } else {
        cpf_param.clone()
    };

    let mut nome_identificado = String::new();

    // 1. Empresas no QSA onde a pessoa física é sócia ou administradora
    let mut empresas_socio: Vec<EmpresaSocioItem> = Vec::new();
    let mut notas_ceap_empresas: Vec<NotaFiscalCeapItem> = Vec::new();
    let mut contratos_pncp_empresas: Vec<ContratoPncpItem> = Vec::new();
    let mut total_ceap_global = storage::Money::ZERO;
    let mut roots_seen = std::collections::HashSet::new();
    let mut total_pncp_global = storage::Money::ZERO;

    {
        let mut qsa_cpf_candidates: Vec<String> = Vec::new();
        if let Some(ref m) = miolo_opt {
            qsa_cpf_candidates.push(format!("***{}**", m));
            qsa_cpf_candidates.push(format!("***.{}.{}-**", &m[0..3], &m[3..6]));
            qsa_cpf_candidates.push(format!("***{}***", m));
        }
        if digits.len() == 11 {
            let m = &digits[3..9];
            let cand1 = format!("***{}**", m);
            if !qsa_cpf_candidates.contains(&cand1) {
                qsa_cpf_candidates.push(cand1);
            }
            let cand2 = format!("***.{}.{}-**", &m[0..3], &m[3..6]);
            if !qsa_cpf_candidates.contains(&cand2) {
                qsa_cpf_candidates.push(cand2);
            }
            if !qsa_cpf_candidates.contains(&digits) {
                qsa_cpf_candidates.push(digits.clone());
            }
        } else if digits.len() == 6 {
            let cand1 = format!("***{}**", digits);
            if !qsa_cpf_candidates.contains(&cand1) {
                qsa_cpf_candidates.push(cand1);
            }
            let cand2 = format!("***.{}.{}-**", &digits[0..3], &digits[3..6]);
            if !qsa_cpf_candidates.contains(&cand2) {
                qsa_cpf_candidates.push(cand2);
            }
        }

        let mut qsa_records = Vec::new();
        if !qsa_cpf_candidates.is_empty() {
            let placeholders = vec!["?"; qsa_cpf_candidates.len()].join(", ");
            let sql_qsa = format!(
                "SELECT cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_nome, qualificacao_socio
                 FROM empresas_qsa
                 WHERE socio_cpf_cnpj_mascarado IN ({})
                 LIMIT 30",
                placeholders
            );
            if let Ok(mut stmt) = conn.prepare(&sql_qsa) {
                let rusqlite_params = rusqlite::params_from_iter(qsa_cpf_candidates.iter());
                if let Ok(rows) = stmt.query_map(rusqlite_params, |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<String>>(5)?,
                    ))
                }) {
                    for r in rows.flatten() {
                        qsa_records.push(r);
                    }
                }
            }
        } else if !termo_nome.is_empty() {
            let termo_fim = format!("{}~", termo_nome);
            if let Ok(mut stmt) = conn.prepare(
                "SELECT cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_nome, qualificacao_socio
                 FROM empresas_qsa
                 WHERE socio_nome >= ?1 AND socio_nome < ?2
                 LIMIT 30",
            ) {
                if let Ok(rows) = stmt.query_map([&termo_nome, &termo_fim], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<String>>(5)?,
                    ))
                }) {
                    for r in rows.flatten() {
                        qsa_records.push(r);
                    }
                }
            }

            if qsa_records.is_empty() {
                if let Ok(mut stmt) = conn.prepare(
                    "SELECT cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_nome, qualificacao_socio
                     FROM empresas_qsa
                     WHERE razao_social >= ?1 AND razao_social < ?2
                     LIMIT 30",
                ) {
                    if let Ok(rows) = stmt.query_map([&termo_nome, &termo_fim], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                            r.get::<_, String>(4)?,
                            r.get::<_, Option<String>>(5)?,
                        ))
                    }) {
                        for r in rows.flatten() {
                            qsa_records.push(r);
                        }
                    }
                }
            }
        }

        for r in qsa_records {
            let c_full = format!("{}{}{}", r.0, r.1, r.2);
            if nome_identificado.is_empty()
                && !r.4.is_empty()
                && r.4 != "HOLDING / SOCIO"
                && !r.4.contains("***")
            {
                nome_identificado = r.4.clone();
            } else if nome_identificado.is_empty() && !r.3.is_empty() {
                nome_identificado = r.3.clone();
            }

            // Consulta CEAP da empresa
            if !roots_seen.insert(r.0.clone()) {
                continue;
            }
            let (total_ceap_emp, _, total_pncp_emp, _) =
                supplier_totals(&conn, &r.0).map_err(database_error)?;
            total_ceap_global = total_ceap_global
                .checked_add(storage::Money::from_reais(total_ceap_emp).map_err(database_error)?)
                .map_err(database_error)?;
            total_pncp_global = total_pncp_global
                .checked_add(storage::Money::from_reais(total_pncp_emp).map_err(database_error)?)
                .map_err(database_error)?;
            let like_cnpj = format!("%{}%", r.0);
            if let Ok(mut stmt_c) = conn.prepare(
                            "SELECT id, parlamentar_nome, data_emissao, categoria_despesa, valor_liquido, numero_documento, url_nota_fiscal, flag_anomalia, detalhes_litros
                             FROM despesas_parlamentares WHERE fornecedor_cnpj_cpf LIKE ?1 LIMIT 20"
                        ) {
                            if let Ok(c_rows) = stmt_c.query_map([&like_cnpj], |nr| {
                                let cat: String = nr.get(3)?;
                                let val: f64 = nr.get(4)?;
                    let litros_opt: Option<f64> = nr.get(8)?;
                    let volume = params_auditoria.volume_combustivel(&cat, val, litros_opt);
                    let volume_estimado = volume.is_some_and(|(_, estimated)| estimated);
                    let volume_litros = volume.map(|(liters, _)| liters);
                    let flag_anomalia = volume_litros.is_some_and(|liters| liters > params_auditoria.limite_combustivel_litros);

                                Ok(NotaFiscalCeapItem {
                                    id: nr.get(0)?,
                                    parlamentar_nome: nr.get(1)?,
                                    data_emissao: nr.get(2)?,
                                    categoria_despesa: cat,
                                    valor_liquido: val,
                                    numero_documento: nr.get(5)?,
                                    url_nota_fiscal: nr.get(6)?,
                                    flag_anomalia,
                        volume_estimado,
                        volume_litros,
                                    politico_id: None,
                                })
                            }) {
                                for nf in c_rows.flatten() {
                                    notas_ceap_empresas.push(nf);
                                }
                            }
                        }

            // Consulta PNCP da empresa

            if let Ok(mut stmt_p) = conn.prepare(
                            "SELECT id, orgao_contratante, valor_contratado, objeto, data_assinatura, data_termino
                             FROM contratos_publicos WHERE fornecedor_cnpj LIKE ?1 LIMIT 10"
                        ) {
                            if let Ok(p_rows) = stmt_p.query_map([&like_cnpj], |pr| {
                                Ok(ContratoPncpItem {
                                    id: pr.get(0)?,
                                    orgao_contratante: pr.get(1)?,
                                    valor_contratado: pr.get(2)?,
                                    objeto: pr.get(3)?,
                                    data_assinatura: pr.get(4)?,
                                    data_termino: pr.get(5)?,
                                })
                            }) {
                                for cp in p_rows.flatten() {
                                    contratos_pncp_empresas.push(cp);
                                }
                            }
                        }

            empresas_socio.push(EmpresaSocioItem {
                cnpj: c_full.clone(),
                cnpj_formatado: formatar_cnpj(&c_full),
                razao_social: r.3,
                qualificacao: r.5,
                total_ceap: total_ceap_emp,
                total_pncp: total_pncp_emp,
            });
        }
    }

    // 2. Doações Eleitorais Feitas no TSE
    let mut doacoes_eleitorais: Vec<DoacaoEleitoralItem> = Vec::new();
    {
        let mut sql_doac = String::from(
            "SELECT rc.id, rc.doador_cpf_cnpj, rc.doador_nome, rc.valor, rc.data_receita,
                    c.ano_eleicao, c.cargo, c.sigla_partido, p.nome_completo, p.id
             FROM receitas_campanha rc
             JOIN candidaturas c ON rc.candidatura_id = c.id
             JOIN politicos p ON c.politico_id = p.id
             WHERE 1=0 ",
        );
        let mut params: Vec<String> = Vec::new();

        if let Some(ref m) = miolo_opt {
            sql_doac.push_str("OR rc.doador_cpf_cnpj LIKE ? OR rc.doador_cpf_cnpj LIKE ? ");
            params.push(format!("%{}%", m));
            params.push(format!("%{}.{}%", &m[0..3], &m[3..6]));
        }
        if !nome_identificado.is_empty() {
            sql_doac.push_str("OR rc.doador_nome LIKE ? ");
            params.push(format!("%{}%", normalizar_nome(&nome_identificado)));
        } else if !termo_nome.is_empty() {
            sql_doac.push_str("OR rc.doador_nome LIKE ? ");
            params.push(format!("%{}%", termo_nome));
        }
        sql_doac.push_str("ORDER BY rc.data_receita DESC LIMIT 30");

        if !params.is_empty() {
            if let Ok(mut stmt) = conn.prepare(&sql_doac) {
                let rusqlite_params = rusqlite::params_from_iter(params.iter());
                if let Ok(rows) = stmt.query_map(rusqlite_params, |r| {
                    Ok(DoacaoEleitoralItem {
                        id: r.get(0)?,
                        doador_cpf_cnpj: r.get(1)?,
                        doador_nome: r.get(2)?,
                        valor: r.get(3)?,
                        data_receita: r.get(4)?,
                        ano_eleicao: r.get(5)?,
                        cargo: r.get(6)?,
                        partido: r.get(7)?,
                        politico_nome: r.get(8)?,
                        politico_id: r.get(9)?,
                    })
                }) {
                    for d in rows.flatten() {
                        if nome_identificado.is_empty() && !d.doador_nome.is_empty() {
                            nome_identificado = d.doador_nome.clone();
                        }
                        doacoes_eleitorais.push(d);
                    }
                }
            }
        }
    }

    // 3. Vínculo como Político ou Candidato (TSE)
    let mut candidaturas: Vec<CandidaturaItemCpf> = Vec::new();
    {
        let mut sql_pol = String::from(
            "SELECT p.id, p.nome_urna, p.nome_completo, c.ano_eleicao, c.cargo, c.sigla_partido, c.uf, c.municipio, c.total_bens_declarados
             FROM politicos p
             JOIN candidaturas c ON c.politico_id = p.id
             WHERE 1=0 ",
        );
        let mut params: Vec<String> = Vec::new();

        if let Some(ref m) = miolo_opt {
            sql_pol.push_str("OR p.cpf_mascarado LIKE ? OR p.cpf_mascarado LIKE ? ");
            params.push(format!("%{}%", m));
            params.push(format!("%{}.{}%", &m[0..3], &m[3..6]));
        }
        if !nome_identificado.is_empty() {
            sql_pol.push_str("OR p.nome_completo LIKE ? ");
            params.push(format!("%{}%", normalizar_nome(&nome_identificado)));
        } else if !termo_nome.is_empty() {
            sql_pol.push_str("OR p.nome_completo LIKE ? ");
            params.push(format!("%{}%", termo_nome));
        }
        sql_pol.push_str("ORDER BY c.ano_eleicao DESC LIMIT 15");

        if !params.is_empty() {
            if let Ok(mut stmt) = conn.prepare(&sql_pol) {
                let rusqlite_params = rusqlite::params_from_iter(params.iter());
                if let Ok(rows) = stmt.query_map(rusqlite_params, |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i32>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, f64>(8)?,
                    ))
                }) {
                    for r in rows.flatten() {
                        if nome_identificado.is_empty() && !r.2.is_empty() {
                            nome_identificado = r.2.clone();
                        }
                        candidaturas.push(CandidaturaItemCpf {
                            politico_id: r.0,
                            nome_urna: r.1,
                            ano_eleicao: r.3,
                            cargo: r.4,
                            partido: r.5,
                            uf: r.6,
                            municipio: r.7,
                            total_bens: r.8,
                        });
                    }
                }
            }
        }
    }

    // 4. Benefícios Emergenciais Recebidos
    let mut beneficios: Vec<BeneficioEmergencialItem> = Vec::new();
    {
        let mut sql_ben = String::from(
            "SELECT id, mes_disponibilizacao, parcela, valor, enquadramento, nome_beneficiario
             FROM beneficios_emergenciais
             WHERE 1=0 ",
        );
        let mut params: Vec<String> = Vec::new();

        if let Some(ref m) = miolo_opt {
            sql_ben.push_str("OR cpf_mascarado LIKE ? OR cpf_mascarado LIKE ? ");
            params.push(format!("%{}%", m));
            params.push(format!("%{}.{}%", &m[0..3], &m[3..6]));
        }
        if !nome_identificado.is_empty() {
            sql_ben.push_str("OR nome_beneficiario LIKE ? ");
            params.push(format!("%{}%", normalizar_nome(&nome_identificado)));
        }
        sql_ben.push_str("LIMIT 15");

        if !params.is_empty() {
            if let Ok(mut stmt) = conn.prepare(&sql_ben) {
                let rusqlite_params = rusqlite::params_from_iter(params.iter());
                if let Ok(rows) = stmt.query_map(rusqlite_params, |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, f64>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, String>(5)?,
                    ))
                }) {
                    for r in rows.flatten() {
                        if nome_identificado.is_empty() && !r.5.is_empty() {
                            nome_identificado = r.5.clone();
                        }
                        beneficios.push(BeneficioEmergencialItem {
                            id: r.0,
                            mes: r.1,
                            parcela: r.2,
                            valor: r.3,
                            enquadramento: r.4,
                        });
                    }
                }
            }
        }
    }

    // 5. Registros Profissionais (OAB/CNA)
    let mut registros_oab: Vec<RegistroProfissionalItem> = Vec::new();
    {
        let mut sql_reg = String::from(
            "SELECT orgao_emissor, numero_registro, seccional_uf, situacao_registro, tipo_inscricao, pessoa_nome
             FROM registros_profissionais
             WHERE 1=0 ",
        );
        let mut params: Vec<String> = Vec::new();

        if let Some(ref m) = miolo_opt {
            sql_reg.push_str("OR cpf_mascarado LIKE ? OR cpf_mascarado LIKE ? ");
            params.push(format!("%{}%", m));
            params.push(format!("%{}.{}%", &m[0..3], &m[3..6]));
        }
        if !nome_identificado.is_empty() {
            sql_reg.push_str("OR pessoa_nome LIKE ? ");
            params.push(format!("%{}%", normalizar_nome(&nome_identificado)));
        }
        sql_reg.push_str("LIMIT 5");

        if !params.is_empty() {
            if let Ok(mut stmt) = conn.prepare(&sql_reg) {
                let rusqlite_params = rusqlite::params_from_iter(params.iter());
                if let Ok(rows) = stmt.query_map(rusqlite_params, |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, String>(5)?,
                    ))
                }) {
                    for r in rows.flatten() {
                        if nome_identificado.is_empty() && !r.5.is_empty() {
                            nome_identificado = r.5.clone();
                        }
                        registros_oab.push(RegistroProfissionalItem {
                            orgao: r.0,
                            numero: r.1,
                            uf: r.2,
                            situacao: r.3,
                            tipo: r.4,
                        });
                    }
                }
            }
        }
    }

    if nome_identificado.is_empty() {
        nome_identificado = if !termo_nome.is_empty() {
            termo_nome
        } else {
            format!("Pessoa Física {}", cpf_formatado)
        };
    }

    // 6. Alertas e Cruzamentos Analíticos
    let mut alertas: Vec<AlertaDossie> = Vec::new();

    // Triangulação: se doou para político que comprou de suas empresas
    for doacao in &doacoes_eleitorais {
        let politico_norm = normalizar_nome(&doacao.politico_nome);
        for nf in &notas_ceap_empresas {
            let comprador_norm = normalizar_nome(&nf.parlamentar_nome);
            if politico_norm == comprador_norm
                || (politico_norm.len() >= 8 && comprador_norm.contains(&politico_norm))
                || (comprador_norm.len() >= 8 && politico_norm.contains(&comprador_norm))
            {
                alertas.push(AlertaDossie {
                    tipo: "TRIANGULACAO_HUB".to_string(),
                    severidade: "CRITICA".to_string(),
                    titulo: "Triangulação Eleitoral / Fornecedor CEAP".to_string(),
                    descricao: format!(
                        "Pessoa física realizou doação eleitoral de R$ {:.2} para {}, e empresa ligada faturou na CEAP do parlamentar.",
                        doacao.valor, doacao.politico_nome
                    ),
                    valor_envolvido: Some(doacao.valor + nf.valor_liquido),
                    fonte: "Cruzamento CEAP + TSE".to_string(),
                });
            }
        }
    }

    // Conflito de Interesses OAB
    let tem_oab_regular = registros_oab
        .iter()
        .any(|r| r.situacao.to_uppercase() == "REGULAR");
    let eh_politico_ativo = candidaturas.iter().any(|c| c.ano_eleicao >= 2020);
    if tem_oab_regular && eh_politico_ativo {
        alertas.push(AlertaDossie {
            tipo: "CONFLITO_OAB".to_string(),
            severidade: "ALTA".to_string(),
            titulo: "Possível Incompatibilidade de Advocacia (Art. 28 EAOAB)".to_string(),
            descricao: "Pessoa física possui inscrição Regular na OAB e ocupa/candidatou-se a cargo público recente.".to_string(),
            valor_envolvido: None,
            fonte: "Cadastro Nacional dos Advogados (CNA/OAB) + TSE".to_string(),
        });
    }

    // Benefício indevido
    let total_beneficios =
        storage::Money::sum_reais(beneficios.iter().map(|b| b.valor)).map_err(database_error)?;
    let total_bens_declarados: f64 = candidaturas
        .iter()
        .map(|c| c.total_bens)
        .fold(0.0, f64::max);
    if total_beneficios > 0.0 && (total_bens_declarados > 300_000.0 || !empresas_socio.is_empty()) {
        alertas.push(AlertaDossie {
            tipo: "AUXILIO_INDEVIDO".to_string(),
            severidade: "ALTA".to_string(),
            titulo: "Benefício Emergencial com Patrimônio ou Empresa Ativa".to_string(),
            descricao: format!(
                "Recebeu R$ {:.2} em benefícios emergenciais constando como sócio de {} empresa(s) ou patrimônio de R$ {:.2}.",
                total_beneficios,
                empresas_socio.len(),
                total_bens_declarados
            ),
            valor_envolvido: Some(total_beneficios),
            fonte: "Cruzamento Portal da Transparência + TSE / RFB".to_string(),
        });
    }

    let score_risco = calcular_score_risco(&alertas);

    Ok(Json(DossieCpfResponse {
        cpf_mascarado: cpf_formatado,
        nome: nome_identificado,
        score_risco,
        total_alertas: alertas.len(),
        alertas,
        empresas_socio,
        total_faturado_empresas_ceap: total_ceap_global.reais(),
        total_contratado_empresas_pncp: total_pncp_global.reais(),
        doacoes_eleitorais,
        candidaturas,
        beneficios_emergenciais: beneficios,
        registros_profissionais: registros_oab,
        notas_ceap_empresas,
        contratos_pncp_empresas,
    }))
}

#[cfg(test)]
mod totals_regressions {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
        routing::get,
        Router,
    };
    use tower::ServiceExt;
    #[tokio::test]
    async fn totais_completos_independem_da_pagina() {
        let pool = DbPool::open_in_memory().unwrap();
        {
            let mut conn = pool.get().unwrap();
            storage::run_migrations(&mut conn).unwrap();
            for i in 0..201 {
                conn.execute("INSERT INTO despesas_parlamentares(casa_legislativa,parlamentar_nome,data_emissao,categoria_despesa,fornecedor_nome,fornecedor_cnpj_cpf,valor_liquido,numero_documento) VALUES ('CAMARA','Parlamentar','2024-01-01','OUTROS','Empresa','12345678000190',?1,?2)",rusqlite::params![(i%2+1) as f64/10.0,format!("NF{i}")]).unwrap();
            }
            for _ in 0..61 {
                conn.execute("INSERT INTO contratos_publicos(orgao_contratante,fornecedor_cnpj,valor_contratado) VALUES ('Prefeitura','12345678000190',0.1)",[]).unwrap();
            }
        }
        let app = Router::new()
            .route("/cnpj/:id", get(dossie_cnpj_handler))
            .with_state(pool);
        let mut totals = None;
        for offset in [0, 150, 201] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!(
                            "/cnpj/12345678000190?ceap_offset={offset}&pncp_offset=50"
                        ))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let dossier: DossieCnpjResponse =
                serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                    .unwrap();
            assert_eq!(dossier.ceap.total_notas, 201);
            assert_eq!(dossier.pncp.total_contratos, 61);
            assert_eq!(dossier.pncp.total_contratado, 6.1);
            assert_eq!(
                dossier.ceap.notas_fiscais.len(),
                if offset == 0 {
                    150
                } else if offset == 150 {
                    51
                } else {
                    0
                }
            );
            assert_eq!(dossier.pncp.contratos.len(), 11);
            assert_eq!(dossier.ceap.compradores[0].quantidade_notas, 201);
            if let Some(total) = totals {
                assert_eq!(dossier.ceap.total_faturado, total);
            } else {
                totals = Some(dossier.ceap.total_faturado);
            }
        }
    }
}
