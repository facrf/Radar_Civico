use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const JANELA_DIAS_FRACIONAMENTO: i64 = 7;
pub const PISO_VALOR_FRACIONAMENTO: f64 = 10_000.0;
pub const MIN_DESPESAS_FRACIONAMENTO: usize = 3;

/// Recebe despesas ordenadas por parlamentar, documento e data. Retém apenas
/// a janela corrente de um fornecedor, em vez da base nacional inteira.
#[derive(Default)]
pub struct AuditorFracionamentoOrdenado {
    janela: std::collections::VecDeque<DespesaFracionamentoInput>,
    anterior: Option<(String, String, NaiveDate)>,
}

impl AuditorFracionamentoOrdenado {
    pub fn adicionar(
        &mut self,
        item: DespesaFracionamentoInput,
    ) -> crate::Result<Vec<AlertaFracionamentoDespesas>> {
        let chave = (
            item.parlamentar_nome.clone(),
            item.fornecedor_cnpj_cpf.clone(),
            item.data_emissao,
        );
        if self
            .anterior
            .as_ref()
            .is_some_and(|anterior| anterior > &chave)
        {
            return Err(crate::AuditorError::Calculation(
                "Despesas fora da ordem de auditoria".into(),
            ));
        }
        self.anterior = Some(chave);
        let mut saida = Vec::new();
        while let Some(primeira) = self.janela.front() {
            let mesmo_grupo = primeira.parlamentar_nome == item.parlamentar_nome
                && primeira.fornecedor_cnpj_cpf == item.fornecedor_cnpj_cpf;
            if mesmo_grupo
                && (item.data_emissao - primeira.data_emissao).num_days()
                    <= JANELA_DIAS_FRACIONAMENTO
            {
                break;
            }
            self.concluir_janela(&mut saida);
        }
        self.janela.push_back(item);
        Ok(saida)
    }

    fn concluir_janela(&mut self, saida: &mut Vec<AlertaFracionamentoDespesas>) {
        let itens: Vec<_> = self.janela.iter().cloned().collect();
        if let Some(alerta) = auditar_fracionamento_despesas(&itens)
            .into_iter()
            .next()
            .filter(|a| a.despesas_ids.first() == itens.first().map(|i| &i.despesa_id))
        {
            // Só conclui o início cuja janela terminou. Um início posterior
            // ainda pode receber despesas na próxima chamada.
            let ultimo = alerta.despesas_ids.last().copied();
            while let Some(item) = self.janela.pop_front() {
                if Some(item.despesa_id) == ultimo {
                    break;
                }
            }
            saida.push(alerta);
        } else {
            self.janela.pop_front();
        }
    }

    pub fn concluir(mut self) -> Vec<AlertaFracionamentoDespesas> {
        let mut saida = Vec::new();
        while !self.janela.is_empty() {
            self.concluir_janela(&mut saida);
        }
        saida
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DespesaFracionamentoInput {
    pub despesa_id: i64,
    pub parlamentar_nome: String,
    pub fornecedor_cnpj_cpf: String,
    pub fornecedor_nome: String,
    pub data_emissao: NaiveDate,
    pub valor: f64,
    pub categoria_despesa: String,
    pub numero_documento: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertaFracionamentoDespesas {
    pub parlamentar_nome: String,
    pub fornecedor_cnpj_cpf: String,
    pub fornecedor_nome: String,
    pub data_inicio: NaiveDate,
    pub data_fim: NaiveDate,
    pub quantidade_despesas: usize,
    pub valor_total: f64,
    pub despesas_ids: Vec<i64>,
    pub gravidade: String,
    pub motivo: String,
}

/// Detecta emissões sucessivas de despesas para o mesmo fornecedor em janela de 7 dias
/// que somem valor expressivo (>= R$ 10.000 com 3+ despesas) ou múltiplas despesas no mesmo dia.
pub fn auditar_fracionamento_despesas(
    despesas: &[DespesaFracionamentoInput],
) -> Vec<AlertaFracionamentoDespesas> {
    let mut grupos: HashMap<(String, String), Vec<&DespesaFracionamentoInput>> = HashMap::new();

    for d in despesas {
        if d.valor <= 0.0 || d.fornecedor_cnpj_cpf.trim().is_empty() {
            continue;
        }
        let parl = d.parlamentar_nome.trim().to_uppercase();
        let forn = d.fornecedor_cnpj_cpf.trim().to_string();
        grupos.entry((parl, forn)).or_default().push(d);
    }

    let mut alertas = Vec::new();

    for ((parl, doc_forn), mut lista) in grupos {
        // Ordena por data crescente
        lista.sort_by_key(|d| d.data_emissao);

        let mut i = 0;
        while i < lista.len() {
            let inicio = lista[i].data_emissao;
            let mut j = i;
            let mut soma = 0.0;
            let mut ids = Vec::new();
            let mut mesmo_dia_count = 0;

            while j < lista.len() {
                let dias = (lista[j].data_emissao - inicio).num_days();
                if dias > JANELA_DIAS_FRACIONAMENTO {
                    break;
                }
                soma += lista[j].valor;
                ids.push(lista[j].despesa_id);
                if lista[j].data_emissao == inicio {
                    mesmo_dia_count += 1;
                }
                j += 1;
            }

            let qtd = ids.len();
            let e_fracionamento_janela =
                qtd >= MIN_DESPESAS_FRACIONAMENTO && soma >= PISO_VALOR_FRACIONAMENTO;
            let e_fracionamento_mesmo_dia = mesmo_dia_count >= 2 && soma >= 8_000.0;

            if e_fracionamento_janela || e_fracionamento_mesmo_dia {
                let forn_nome = &lista[i].fornecedor_nome;
                let data_fim = lista[j - 1].data_emissao;
                let gravidade = if soma >= 30_000.0 || qtd >= 5 {
                    "CRITICA"
                } else {
                    "ALTA"
                };

                let motivo = if inicio == data_fim {
                    format!(
                        "Parlamentar {} emitiu {} despesas no mesmo dia ({}) para o fornecedor {} ({}), totalizando R$ {:.2} (indício de fracionamento de compras).",
                        parl, qtd, inicio.format("%d/%m/%Y"), forn_nome, doc_forn, soma
                    )
                } else {
                    format!(
                        "Parlamentar {} realizou {} pagamentos concentrados entre {} e {} para o fornecedor {} ({}), somando R$ {:.2} (indício de fracionamento de despesas na cota).",
                        parl, qtd, inicio.format("%d/%m/%Y"), data_fim.format("%d/%m/%Y"), forn_nome, doc_forn, soma
                    )
                };

                alertas.push(AlertaFracionamentoDespesas {
                    parlamentar_nome: parl.clone(),
                    fornecedor_cnpj_cpf: doc_forn.clone(),
                    fornecedor_nome: forn_nome.clone(),
                    data_inicio: inicio,
                    data_fim,
                    quantidade_despesas: qtd,
                    valor_total: (soma * 100.0).round() / 100.0,
                    despesas_ids: ids,
                    gravidade: gravidade.to_string(),
                    motivo,
                });

                i = j; // Avança para além do bloco detectado para não duplicar alertas
            } else {
                i += 1;
            }
        }
    }

    alertas.sort_by(|a, b| {
        b.valor_total
            .partial_cmp(&a.valor_total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    alertas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fracionamento_mesmo_dia() {
        let d1 = DespesaFracionamentoInput {
            despesa_id: 101,
            parlamentar_nome: "DEPUTADO A".into(),
            fornecedor_cnpj_cpf: "12345678000199".into(),
            fornecedor_nome: "PAPELARIA MODELO".into(),
            data_emissao: NaiveDate::from_ymd_opt(2024, 5, 10).unwrap(),
            valor: 4_500.0,
            categoria_despesa: "MATERIAL DE ESCRITORIO".into(),
            numero_documento: Some("NF-1".into()),
        };
        let d2 = DespesaFracionamentoInput {
            despesa_id: 102,
            parlamentar_nome: "DEPUTADO A".into(),
            fornecedor_cnpj_cpf: "12345678000199".into(),
            fornecedor_nome: "PAPELARIA MODELO".into(),
            data_emissao: NaiveDate::from_ymd_opt(2024, 5, 10).unwrap(),
            valor: 4_200.0,
            categoria_despesa: "MATERIAL DE ESCRITORIO".into(),
            numero_documento: Some("NF-2".into()),
        };

        let alertas = auditar_fracionamento_despesas(&[d1, d2]);
        assert_eq!(alertas.len(), 1);
        let a = &alertas[0];
        assert_eq!(a.quantidade_despesas, 2);
        assert_eq!(a.valor_total, 8_700.0);
        assert_eq!(a.gravidade, "ALTA");
        assert!(a.motivo.contains("mesmo dia"));
    }

    #[test]
    fn test_fracionamento_janela_7_dias() {
        let despesas = vec![
            DespesaFracionamentoInput {
                despesa_id: 1,
                parlamentar_nome: "DEPUTADO B".into(),
                fornecedor_cnpj_cpf: "99887766000111".into(),
                fornecedor_nome: "CONSULTORIA ABC".into(),
                data_emissao: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
                valor: 5_000.0,
                categoria_despesa: "CONSULTORIA".into(),
                numero_documento: None,
            },
            DespesaFracionamentoInput {
                despesa_id: 2,
                parlamentar_nome: "DEPUTADO B".into(),
                fornecedor_cnpj_cpf: "99887766000111".into(),
                fornecedor_nome: "CONSULTORIA ABC".into(),
                data_emissao: NaiveDate::from_ymd_opt(2024, 6, 3).unwrap(),
                valor: 5_000.0,
                categoria_despesa: "CONSULTORIA".into(),
                numero_documento: None,
            },
            DespesaFracionamentoInput {
                despesa_id: 3,
                parlamentar_nome: "DEPUTADO B".into(),
                fornecedor_cnpj_cpf: "99887766000111".into(),
                fornecedor_nome: "CONSULTORIA ABC".into(),
                data_emissao: NaiveDate::from_ymd_opt(2024, 6, 5).unwrap(),
                valor: 5_000.0,
                categoria_despesa: "CONSULTORIA".into(),
                numero_documento: None,
            },
        ];

        let alertas = auditar_fracionamento_despesas(&despesas);
        assert_eq!(alertas.len(), 1);
        assert_eq!(alertas[0].quantidade_despesas, 3);
        assert_eq!(alertas[0].valor_total, 15_000.0);
    }
}

#[cfg(test)]
mod streaming_regressions {
    use super::*;
    fn item(id: i64, dia: i64, valor: f64) -> DespesaFracionamentoInput {
        DespesaFracionamentoInput {
            despesa_id: id,
            parlamentar_nome: "PARLAMENTAR".into(),
            fornecedor_cnpj_cpf: "52998224725".into(),
            fornecedor_nome: "FORNECEDOR".into(),
            data_emissao: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()
                + chrono::Duration::days(dia),
            valor,
            categoria_despesa: "SERVICOS".into(),
            numero_documento: Some(id.to_string()),
        }
    }
    fn comparar(itens: Vec<DespesaFracionamentoInput>) {
        let mut referencia = auditar_fracionamento_despesas(&itens);
        let mut motor = AuditorFracionamentoOrdenado::default();
        let mut resultado = Vec::new();
        for item in itens {
            resultado.extend(motor.adicionar(item).unwrap());
        }
        resultado.extend(motor.concluir());
        referencia.sort_by_key(|a| (a.parlamentar_nome.clone(), a.data_inicio));
        resultado.sort_by_key(|a| (a.parlamentar_nome.clone(), a.data_inicio));
        assert_eq!(resultado, referencia);
    }
    #[test]
    fn janela_posterior_aguarda_despesas_que_ainda_podem_chegar() {
        comparar(vec![
            item(1, 0, 1000.0),
            item(2, 1, 4000.0),
            item(3, 1, 4000.0),
            item(4, 8, 5000.0),
        ]);
    }
    #[test]
    fn fluxo_ordenado_equivale_a_lotes_em_varias_janelas_e_grupos() {
        let mut itens = Vec::new();
        for grupo in 0..5 {
            for i in 0..90 {
                let mut entrada = item(
                    grupo * 100 + i,
                    i / 2,
                    ((i * 7919 + grupo * 101) % 5000) as f64 + 0.01,
                );
                entrada.parlamentar_nome = format!("PARLAMENTAR {grupo}");
                itens.push(entrada);
            }
        }
        comparar(itens);
        let mut motor = AuditorFracionamentoOrdenado::default();
        motor.adicionar(item(2, 2, 10.0)).unwrap();
        assert!(motor.adicionar(item(1, 1, 10.0)).is_err());
    }
}
