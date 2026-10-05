use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const LIMITE_VELOCIDADE_KMH: f64 = 800.0;
const RAIO_TERRA_KM: f64 = 6371.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DespesaPresencial {
    pub id: i64,
    pub parlamentar_nome: String,
    pub timestamp: DateTime<Utc>,
    pub municipio: String,
    pub uf: String,
    pub latitude: f64,
    pub longitude: f64,
    pub valor: f64,
    pub descricao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaUbiquidade {
    pub despesa_origem_id: i64,
    pub despesa_destino_id: i64,
    pub parlamentar_nome: String,
    pub distancia_km: f64,
    pub tempo_horas: f64,
    pub velocidade_kmh: f64,
    pub motivo: String,
}

pub fn calcular_distancia_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();

    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    RAIO_TERRA_KM * c
}

pub fn auditar_ubiquidade(despesas: &[DespesaPresencial]) -> Vec<AlertaUbiquidade> {
    if despesas.len() < 2 {
        return Vec::new();
    }

    let mut ordenadas = despesas.to_vec();
    ordenadas.sort_by(|a, b| {
        a.parlamentar_nome
            .cmp(&b.parlamentar_nome)
            .then_with(|| a.timestamp.cmp(&b.timestamp))
    });

    let mut alertas = Vec::new();

    for janela in ordenadas.windows(2) {
        let d1 = &janela[0];
        let d2 = &janela[1];

        if d1.parlamentar_nome != d2.parlamentar_nome {
            continue;
        }

        let distancia = calcular_distancia_km(d1.latitude, d1.longitude, d2.latitude, d2.longitude);
        let segundos = (d2.timestamp - d1.timestamp).num_seconds();

        if segundos <= 0 {
            if distancia > 1.0 {
                alertas.push(AlertaUbiquidade {
                    despesa_origem_id: d1.id,
                    despesa_destino_id: d2.id,
                    parlamentar_nome: d1.parlamentar_nome.clone(),
                    distancia_km: (distancia * 100.0).round() / 100.0,
                    tempo_horas: 0.0,
                    velocidade_kmh: f64::INFINITY,
                    motivo: format!(
                        "Despesas simultâneas emitidas em locais distantes ({:.1} km) no mesmo instante",
                        distancia
                    ),
                });
            }
            continue;
        }

        let horas = segundos as f64 / 3600.0;
        let velocidade = distancia / horas;

        if velocidade > LIMITE_VELOCIDADE_KMH {
            alertas.push(AlertaUbiquidade {
                despesa_origem_id: d1.id,
                despesa_destino_id: d2.id,
                parlamentar_nome: d1.parlamentar_nome.clone(),
                distancia_km: (distancia * 100.0).round() / 100.0,
                tempo_horas: (horas * 100.0).round() / 100.0,
                velocidade_kmh: (velocidade * 100.0).round() / 100.0,
                motivo: format!(
                    "Deslocamento fisicamente impossível entre {}/{} e {}/{}: {:.1} km em {:.2} h ({:.1} km/h > {:.0} km/h)",
                    d1.municipio, d1.uf, d2.municipio, d2.uf, distancia, horas, velocidade, LIMITE_VELOCIDADE_KMH
                ),
            });
        }
    }

    alertas
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_ubiquidade_detecta_velocidade_super_sonica() {
        let t1 = Utc.with_ymd_and_hms(2024, 4, 10, 12, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 4, 10, 13, 0, 0).unwrap(); // 1 hora depois

        let despesas = vec![
            DespesaPresencial {
                id: 1,
                parlamentar_nome: "DEPUTADO VIAJANTE".to_string(),
                timestamp: t1,
                municipio: "São Paulo".to_string(),
                uf: "SP".to_string(),
                latitude: -23.5505,
                longitude: -46.6333,
                valor: 150.0,
                descricao: "Almoço em São Paulo".to_string(),
            },
            DespesaPresencial {
                id: 2,
                parlamentar_nome: "DEPUTADO VIAJANTE".to_string(),
                timestamp: t2,
                municipio: "Fortaleza".to_string(),
                uf: "CE".to_string(),
                latitude: -3.7172,
                longitude: -38.5433,
                valor: 200.0,
                descricao: "Jantar em Fortaleza".to_string(),
            },
        ];

        let alertas = auditar_ubiquidade(&despesas);
        assert_eq!(alertas.len(), 1);
        assert_eq!(alertas[0].parlamentar_nome, "DEPUTADO VIAJANTE");
        assert!(alertas[0].velocidade_kmh > 2000.0);
        assert!(alertas[0].motivo.contains("fisicamente impossível"));
    }

    #[test]
    fn test_ubiquidade_caso_controle_sem_falso_positivo() {
        let t1 = Utc.with_ymd_and_hms(2024, 4, 10, 8, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 4, 10, 14, 0, 0).unwrap(); // 6 horas depois

        // SP para Rio (~360 km em 6 horas = 60 km/h)
        let despesas = vec![
            DespesaPresencial {
                id: 3,
                parlamentar_nome: "DEPUTADO NORMAL".to_string(),
                timestamp: t1,
                municipio: "São Paulo".to_string(),
                uf: "SP".to_string(),
                latitude: -23.5505,
                longitude: -46.6333,
                valor: 50.0,
                descricao: "Café da manhã".to_string(),
            },
            DespesaPresencial {
                id: 4,
                parlamentar_nome: "DEPUTADO NORMAL".to_string(),
                timestamp: t2,
                municipio: "Rio de Janeiro".to_string(),
                uf: "RJ".to_string(),
                latitude: -22.9068,
                longitude: -43.1729,
                valor: 80.0,
                descricao: "Almoço".to_string(),
            },
        ];

        let alertas = auditar_ubiquidade(&despesas);
        assert!(alertas.is_empty());
    }

    #[test]
    fn test_ubiquidade_parlamentares_diferentes_nao_colidem() {
        let t = Utc.with_ymd_and_hms(2024, 4, 10, 12, 0, 0).unwrap();

        let despesas = vec![
            DespesaPresencial {
                id: 5,
                parlamentar_nome: "DEPUTADO A".to_string(),
                timestamp: t,
                municipio: "São Paulo".to_string(),
                uf: "SP".to_string(),
                latitude: -23.5505,
                longitude: -46.6333,
                valor: 100.0,
                descricao: "Despesa A".to_string(),
            },
            DespesaPresencial {
                id: 6,
                parlamentar_nome: "DEPUTADO B".to_string(),
                timestamp: t,
                municipio: "Manaus".to_string(),
                uf: "AM".to_string(),
                latitude: -3.1190,
                longitude: -60.0217,
                valor: 100.0,
                descricao: "Despesa B".to_string(),
            },
        ];

        let alertas = auditar_ubiquidade(&despesas);
        assert!(alertas.is_empty());
    }
}
