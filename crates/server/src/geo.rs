use storage::rusqlite::Connection;

#[derive(Debug, Clone, PartialEq)]
pub struct CoordenadaResolvida {
    pub municipio: String,
    pub uf: String,
    pub latitude: f64,
    pub longitude: f64,
    pub fora_uf_origem: bool,
    pub alerta_distancia: bool,
    pub distancia_origem_km: f64,
    pub motivo_alerta: Option<String>,
}

/// Calcula a distância em quilômetros entre dois pontos geográficos usando a fórmula de Haversine.
pub fn calcular_distancia_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0; // Raio médio da Terra em km
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();

    let a = (d_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    (r * c * 10.0).round() / 10.0
}

/// Retorna as coordenadas da capital e nome da capital para uma determinada UF brasileira.
pub fn coordenadas_capital_uf(uf: &str) -> Option<(&'static str, f64, f64)> {
    match uf.to_uppercase().trim() {
        "AC" => Some(("Rio Branco", -9.9753, -67.8249)),
        "AL" => Some(("Maceió", -9.6658, -35.7353)),
        "AP" => Some(("Macapá", 0.0355, -51.0705)),
        "AM" => Some(("Manaus", -3.1190, -60.0217)),
        "BA" => Some(("Salvador", -12.9777, -38.5016)),
        "CE" => Some(("Fortaleza", -3.7319, -38.5267)),
        "DF" => Some(("Brasília", -15.7939, -47.8828)),
        "ES" => Some(("Vitória", -20.3155, -40.3128)),
        "GO" => Some(("Goiânia", -16.6869, -49.2648)),
        "MA" => Some(("São Luís", -2.5307, -44.3068)),
        "MT" => Some(("Cuiabá", -15.6014, -56.0979)),
        "MS" => Some(("Campo Grande", -20.4697, -54.6201)),
        "MG" => Some(("Belo Horizonte", -19.9208, -43.9378)),
        "PA" => Some(("Belém", -1.4558, -48.4902)),
        "PB" => Some(("João Pessoa", -7.1195, -34.8450)),
        "PR" => Some(("Curitiba", -25.4289, -49.2671)),
        "PE" => Some(("Recife", -8.0476, -34.8770)),
        "PI" => Some(("Teresina", -5.0919, -42.8034)),
        "RJ" => Some(("Rio de Janeiro", -22.9068, -43.1729)),
        "RN" => Some(("Natal", -5.7945, -35.2110)),
        "RS" => Some(("Porto Alegre", -30.0346, -51.2177)),
        "RO" => Some(("Porto Velho", -8.7619, -63.9039)),
        "RR" => Some(("Boa Vista", 2.8235, -60.6758)),
        "SC" => Some(("Florianópolis", -27.5954, -48.5481)),
        "SP" => Some(("São Paulo", -23.5505, -46.6333)),
        "SE" => Some(("Aracaju", -10.9472, -37.0731)),
        "TO" => Some(("Palmas", -10.2491, -48.3243)),
        _ => None,
    }
}

/// Consulta o banco de dados SQLite para obter coordenadas de município/UF.
pub fn buscar_coordenadas_municipio(
    conn: &Connection,
    municipio: &str,
    uf: &str,
) -> Option<(f64, f64)> {
    let mut stmt = conn
        .prepare_cached("SELECT latitude, longitude FROM municipios_ibge WHERE uf = ?1 AND UPPER(nome) = UPPER(?2) LIMIT 1")
        .ok()?;
    stmt.query_row([uf, municipio], |row| Ok((row.get(0)?, row.get(1)?)))
        .ok()
        .or_else(|| {
            // Tenta busca parcial se o nome contiver o termo
            let mut stmt_like = conn
                .prepare_cached("SELECT latitude, longitude FROM municipios_ibge WHERE uf = ?1 AND UPPER(nome) LIKE UPPER(?2) LIMIT 1")
                .ok()?;
            stmt_like
                .query_row([uf, &format!("%{}%", municipio)], |row| Ok((row.get(0)?, row.get(1)?)))
                .ok()
        })
        .or_else(|| {
            // Fallback para coordenadas da capital da UF
            coordenadas_capital_uf(uf).map(|(_, lat, lon)| (lat, lon))
        })
}

/// Resolve a localização geográfica aproximada de um fornecedor e despesa parlamentar.
pub fn resolver_coordenadas_despesa(
    _conn: &Connection,
    fornecedor_nome: &str,
    fornecedor_cnpj: &str,
    categoria_despesa: &str,
    valor_liquido: f64,
    uf_politico: &str,
) -> CoordenadaResolvida {
    let nome_upper = fornecedor_nome.to_uppercase();
    let _cat_upper = categoria_despesa.to_uppercase();

    // 1. Identificação de Brasília / DF (sede do Congresso Nacional)
    let is_df = nome_upper.contains("CASCOL")
        || nome_upper.contains("BRASILIA")
        || nome_upper.contains("BRASÍLIA")
        || nome_upper.contains("BSB")
        || nome_upper.contains("ASA NORTE")
        || nome_upper.contains("ASA SUL")
        || nome_upper.contains("LAGO NORTE")
        || nome_upper.contains("LAGO SUL")
        || nome_upper.contains("PARANOA")
        || nome_upper.contains("TAGUATINGA")
        || nome_upper.contains("CEILANDIA")
        || nome_upper.contains("302 NORTE")
        || nome_upper.contains("306 NORTE")
        || nome_upper.contains("311 NORTE")
        || nome_upper.contains("405 SUL")
        || nome_upper.contains("SENADO")
        || nome_upper.contains("CAMARA DOS DEPUTADOS");

    let (municipio_detectado, uf_detectada, base_lat, base_lon) = if is_df {
        ("Brasília".to_string(), "DF".to_string(), -15.793889, -47.882778)
    } else {
        // 2. Busca por municípios conhecidos no nome do estabelecimento
        let municipios_notaveis = [
            ("CAXIAS DO SUL", "RS", -29.1678, -51.1794),
            ("GALOPOLIS", "RS", -29.2312, -51.1643),
            ("PELOTAS", "RS", -31.7654, -52.3376),
            ("SANTA MARIA", "RS", -29.6842, -53.8069),
            ("CANOAS", "RS", -29.9178, -51.1836),
            ("PORTO ALEGRE", "RS", -30.0346, -51.2177),
            ("CAMPINAS", "SP", -22.9099, -47.0626),
            ("SANTOS", "SP", -23.9608, -46.3336),
            ("RIBEIRAO PRETO", "SP", -21.1775, -47.8103),
            ("RIBEIRÃO PRETO", "SP", -21.1775, -47.8103),
            ("SAO PAULO", "SP", -23.5505, -46.6333),
            ("SÃO PAULO", "SP", -23.5505, -46.6333),
            ("RIO DE JANEIRO", "RJ", -22.9068, -43.1729),
            ("NITEROI", "RJ", -22.8833, -43.1036),
            ("NITERÓI", "RJ", -22.8833, -43.1036),
            ("BELO HORIZONTE", "MG", -19.9208, -43.9378),
            ("UBERLANDIA", "MG", -18.9186, -48.2772),
            ("UBERLÂNDIA", "MG", -18.9186, -48.2772),
            ("JUIZ DE FORA", "MG", -21.7642, -43.3497),
            ("CURITIBA", "PR", -25.4289, -49.2671),
            ("LONDRINA", "PR", -23.3045, -51.1696),
            ("MARINGA", "PR", -23.4209, -51.9331),
            ("MARINGÁ", "PR", -23.4209, -51.9331),
            ("JOINVILLE", "SC", -26.3045, -48.8487),
            ("FLORIANOPOLIS", "SC", -27.5954, -48.5481),
            ("FLORIANÓPOLIS", "SC", -27.5954, -48.5481),
            ("SALVADOR", "BA", -12.9777, -38.5016),
            ("FEIRA DE SANTANA", "BA", -12.2667, -38.9667),
            ("RECIFE", "PE", -8.0476, -34.8770),
            ("FORTALEZA", "CE", -3.7319, -38.5267),
            ("GOIANIA", "GO", -16.6869, -49.2648),
            ("GOIÂNIA", "GO", -16.6869, -49.2648),
            ("ANAPOLIS", "GO", -16.3267, -48.9533),
            ("ANÁPOLIS", "GO", -16.3267, -48.9533),
            ("CUIABA", "MT", -15.6014, -56.0979),
            ("CUIABÁ", "MT", -15.6014, -56.0979),
            ("CAMPO GRANDE", "MS", -20.4697, -54.6201),
            ("BELEM", "PA", -1.4558, -48.4902),
            ("BELÉM", "PA", -1.4558, -48.4902),
            ("MANAUS", "AM", -3.1190, -60.0217),
        ];

        let mut encontrado = None;
        for (m_nome, m_uf, m_lat, m_lon) in municipios_notaveis {
            if nome_upper.contains(m_nome) {
                encontrado = Some((m_nome.to_string(), m_uf.to_string(), m_lat, m_lon));
                break;
            }
        }

        if let Some(mun) = encontrado {
            mun
        } else {
            // 3. Fallback: Se for fornecedor local do estado de origem
            if let Some((cap_nome, cap_lat, cap_lon)) = coordenadas_capital_uf(uf_politico) {
                (cap_nome.to_string(), uf_politico.to_string(), cap_lat, cap_lon)
            } else {
                ("Brasília".to_string(), "DF".to_string(), -15.793889, -47.882778)
            }
        }
    };

    // Micro-offset pseudo-aleatório determinístico baseado no CNPJ/nome
    // para evitar sobreposição exata de múltiplos estabelecimentos na mesma cidade
    let seed = fornecedor_cnpj
        .chars()
        .filter(|c| c.is_ascii_digit())
        .fold(17u64, |acc, c| acc.wrapping_mul(37).wrapping_add(c as u64));

    let offset_lat = (((seed % 1000) as f64 - 500.0) / 100000.0) * 1.5;
    let offset_lon = ((((seed / 1000) % 1000) as f64 - 500.0) / 100000.0) * 1.5;

    let latitude = ((base_lat + offset_lat) * 10000.0).round() / 10000.0;
    let longitude = ((base_lon + offset_lon) * 10000.0).round() / 10000.0;

    // Distância da base eleitoral (capital da UF de origem do político)
    let (origem_lat, origem_lon) = coordenadas_capital_uf(uf_politico)
        .map(|(_, lat, lon)| (lat, lon))
        .unwrap_or((-15.793889, -47.882778));

    let distancia_origem_km = calcular_distancia_km(origem_lat, origem_lon, latitude, longitude);

    // Avaliação de alerta fora da UF:
    // Deputados federais têm exercício parlamentar legítimo em Brasília (DF).
    // Despesas fora da sua UF de origem e fora do DF são marcadas como fora da UF.
    let fora_uf_origem = uf_detectada != uf_politico && uf_detectada != "DF";
    let alerta_distancia = fora_uf_origem && distancia_origem_km > 250.0;

    let motivo_alerta = if alerta_distancia {
        Some(format!(
            "Despesa de R$ {:.2} ({}) realizada em {}/{} a {:.0} km da base eleitoral ({})",
            valor_liquido, categoria_despesa, municipio_detectado, uf_detectada, distancia_origem_km, uf_politico
        ))
    } else {
        None
    };

    CoordenadaResolvida {
        municipio: municipio_detectado,
        uf: uf_detectada,
        latitude,
        longitude,
        fora_uf_origem,
        alerta_distancia,
        distancia_origem_km,
        motivo_alerta,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[test]
    fn test_calculo_distancia_haversine() {
        // Distância entre Brasília (-15.7939, -47.8828) e Porto Alegre (-30.0346, -51.2177)
        let dist = calcular_distancia_km(-15.7939, -47.8828, -30.0346, -51.2177);
        assert!(dist > 1600.0 && dist < 1650.0, "Distância BSB-POA deve ser ~1615km, deu: {}", dist);
    }

    #[test]
    fn test_resolucao_coordenadas_despesa() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // 1. Despesa de posto Cascol em Brasília para deputado do RS
        let res_df = resolver_coordenadas_despesa(
            &conn,
            "031 - 302 NORTE - CASCOL COMBUSTIVEIS",
            "00306597003112",
            "COMBUSTÍVEIS E LUBRIFICANTES.",
            150.0,
            "RS",
        );
        assert_eq!(res_df.uf, "DF");
        assert_eq!(res_df.municipio, "Brasília");
        assert!(!res_df.fora_uf_origem, "DF não deve ser considerado fora_uf_origem de deputado federal");
        assert!(!res_df.alerta_distancia);

        // 2. Despesa em Salvador/BA para deputado do RS (muito distante, fora da UF e do DF)
        let res_ba = resolver_coordenadas_despesa(
            &conn,
            "POSTO PRAIA SALVADOR LTDA",
            "12345678000199",
            "COMBUSTÍVEIS E LUBRIFICANTES.",
            400.0,
            "RS",
        );
        assert_eq!(res_ba.uf, "BA");
        assert_eq!(res_ba.municipio, "SALVADOR");
        assert!(res_ba.fora_uf_origem);
        assert!(res_ba.alerta_distancia);
        assert!(res_ba.distancia_origem_km > 2000.0);
        assert!(res_ba.motivo_alerta.is_some());
    }
}
