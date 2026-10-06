use std::collections::{HashMap, HashSet, VecDeque};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};

use crate::builder::GrafoSincronizado;

pub const LIMITE_CONCENTRACAO_COLIGACAO: f64 = 70.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FornecedorHubAlerta {
    pub fornecedor_id: i64,
    pub fornecedor_uuid: String,
    pub fornecedor_nome: String,
    pub total_faturamento: f64,
    pub maior_coligacao_grupo: String,
    pub faturamento_grupo: f64,
    pub percentual_concentracao: f64,
    pub betweenness_score: f64,
    pub quantidade_agentes_conectados: usize,
    pub motivo: String,
}

impl GrafoSincronizado {
    /// Calcula a Betweenness Centrality de todos os nós no grafo utilizando o algoritmo de Brandes
    pub fn calcular_betweenness_centrality(&self) -> HashMap<NodeIndex, f64> {
        let mut cb: HashMap<NodeIndex, f64> = self
            .grafo
            .node_indices()
            .map(|idx| (idx, 0.0))
            .collect();

        let node_indices: Vec<NodeIndex> = self.grafo.node_indices().collect();

        for &s in &node_indices {
            let mut stack = Vec::new();
            let mut p: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
            let mut sigma: HashMap<NodeIndex, f64> = HashMap::new();
            let mut d: HashMap<NodeIndex, i32> = HashMap::new();

            for &v in &node_indices {
                p.insert(v, Vec::new());
                sigma.insert(v, 0.0);
                d.insert(v, -1);
            }

            sigma.insert(s, 1.0);
            d.insert(s, 0);

            let mut q = VecDeque::new();
            q.push_back(s);

            while let Some(v) = q.pop_front() {
                stack.push(v);
                let d_v = d[&v];

                for edge in self.grafo.edges_directed(v, Direction::Outgoing) {
                    let w = edge.target();
                    // Primeiro encontro com w
                    if d[&w] < 0 {
                        d.insert(w, d_v + 1);
                        q.push_back(w);
                    }
                    // Caminho mais curto para w passando por v
                    if d[&w] == d_v + 1 {
                        let sigma_v = sigma[&v];
                        *sigma.get_mut(&w).unwrap() += sigma_v;
                        p.get_mut(&w).unwrap().push(v);
                    }
                }
            }

            let mut delta: HashMap<NodeIndex, f64> = node_indices
                .iter()
                .map(|&idx| (idx, 0.0))
                .collect();

            while let Some(w) = stack.pop() {
                for &v in &p[&w] {
                    let c = (sigma[&v] / sigma[&w]) * (1.0 + delta[&w]);
                    *delta.get_mut(&v).unwrap() += c;
                }
                if w != s {
                    *cb.get_mut(&w).unwrap() += delta[&w];
                }
            }
        }

        cb
    }

    /// Detecta fornecedores do tipo "Hub" com concentração > 70% em um único grupo/coligação
    pub fn detectar_fornecedores_hub(&self) -> Vec<FornecedorHubAlerta> {
        self.detectar_fornecedores_hub_com_limite(LIMITE_CONCENTRACAO_COLIGACAO)
    }

    /// Detecta fornecedores do tipo "Hub" com concentração >= limite configurado em um único grupo/coligação
    pub fn detectar_fornecedores_hub_com_limite(&self, limite_concentracao: f64) -> Vec<FornecedorHubAlerta> {
        let betweenness = self.calcular_betweenness_centrality();
        let mut alertas = Vec::new();

        for node_idx in self.grafo.node_indices() {
            let no = match self.grafo.node_weight(node_idx) {
                Some(n) => n,
                None => continue,
            };

            if no.tipo != "EMPRESA" {
                continue;
            }

            // Agrupa faturamento por grupo/coligação/partido dos pagadores
            let mut faturamento_por_grupo: HashMap<String, f64> = HashMap::new();
            let mut total_faturamento = 0.0;
            let mut agentes_conectados = HashSet::new();

            for edge in self.grafo.edges_directed(node_idx, Direction::Incoming) {
                let pagador_idx = edge.source();
                let aresta = edge.weight();
                agentes_conectados.insert(pagador_idx);

                let pagador = self.grafo.node_weight(pagador_idx);
                let grupo = pagador
                    .and_then(|p| {
                        // Tenta extrair grupo de metadata_json ou esfera/partido
                        if let Some(meta) = &p.metadata_json {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(meta) {
                                if let Some(colig) = v.get("coligacao").and_then(|c| c.as_str()) {
                                    return Some(colig.to_string());
                                }
                                if let Some(part) = v.get("partido").and_then(|c| c.as_str()) {
                                    return Some(part.to_string());
                                }
                            }
                        }
                        p.esfera.clone()
                    })
                    .unwrap_or_else(|| "GRUPO_INDEFINIDO".to_string());

                *faturamento_por_grupo.entry(grupo).or_insert(0.0) += aresta.valor;
                total_faturamento += aresta.valor;
            }

            if total_faturamento <= 0.0 || agentes_conectados.len() < 2 {
                continue;
            }

            // Encontra grupo com maior concentração
            if let Some((maior_grupo, &faturamento_grupo)) = faturamento_por_grupo
                .iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            {
                let percentual = (faturamento_grupo / total_faturamento) * 100.0;
                let score = *betweenness.get(&node_idx).unwrap_or(&0.0);

                if percentual >= limite_concentracao {
                    alertas.push(FornecedorHubAlerta {
                        fornecedor_id: no.db_id,
                        fornecedor_uuid: no.uuid.clone(),
                        fornecedor_nome: no.nome.clone(),
                        total_faturamento,
                        maior_coligacao_grupo: maior_grupo.clone(),
                        faturamento_grupo,
                        percentual_concentracao: (percentual * 100.0).round() / 100.0,
                        betweenness_score: (score * 100.0).round() / 100.0,
                        quantidade_agentes_conectados: agentes_conectados.len(),
                        motivo: format!(
                            "Fornecedor 'Hub' identificado: {:.1}% do faturamento (R$ {:.2} de R$ {:.2}) canalizado por um único grupo ({}) através de {} agentes.",
                            percentual, faturamento_grupo, total_faturamento, maior_grupo, agentes_conectados.len()
                        ),
                    });
                }
            }
        }

        alertas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[test]
    fn test_hub_detecta_concentracao_acima_70_porcento() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // 1 Fornecedor (EMPRESA)
        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('emp-hub', 'EMPRESA', 'GRAFICA CENTRAL HUB')",
            [],
        ).unwrap();
        let emp_id = conn.last_insert_rowid();

        // 3 Candidatos da Coligacao A
        let meta_a = serde_json::json!({ "coligacao": "COLIGACAO PRA FRENTE" }).to_string();
        conn.execute("INSERT INTO nos_rede (uuid, tipo, nome, metadata_json) VALUES ('c1', 'POLITICO', 'CAND 1', ?1)", [&meta_a]).unwrap();
        let c1 = conn.last_insert_rowid();
        conn.execute("INSERT INTO nos_rede (uuid, tipo, nome, metadata_json) VALUES ('c2', 'POLITICO', 'CAND 2', ?1)", [&meta_a]).unwrap();
        let c2 = conn.last_insert_rowid();
        conn.execute("INSERT INTO nos_rede (uuid, tipo, nome, metadata_json) VALUES ('c3', 'POLITICO', 'CAND 3', ?1)", [&meta_a]).unwrap();
        let c3 = conn.last_insert_rowid();

        // 1 Candidato de outro grupo
        let meta_b = serde_json::json!({ "coligacao": "OPOSICAO" }).to_string();
        conn.execute("INSERT INTO nos_rede (uuid, tipo, nome, metadata_json) VALUES ('c4', 'POLITICO', 'CAND 4', ?1)", [&meta_b]).unwrap();
        let c4 = conn.last_insert_rowid();

        // Repasses: c1 -> emp (30k), c2 -> emp (30k), c3 -> emp (30k) = 90k da Coligação A
        // c4 -> emp (10k) da Oposição. Total = 100k (90% de concentração!)
        conn.execute("INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado) VALUES (?1, ?2, 'DESPESA', 30000.0, 2024, 'TSE')", storage::rusqlite::params![c1, emp_id]).unwrap();
        conn.execute("INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado) VALUES (?1, ?2, 'DESPESA', 30000.0, 2024, 'TSE')", storage::rusqlite::params![c2, emp_id]).unwrap();
        conn.execute("INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado) VALUES (?1, ?2, 'DESPESA', 30000.0, 2024, 'TSE')", storage::rusqlite::params![c3, emp_id]).unwrap();
        conn.execute("INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado) VALUES (?1, ?2, 'DESPESA', 10000.0, 2024, 'TSE')", storage::rusqlite::params![c4, emp_id]).unwrap();

        let grafo = GrafoSincronizado::carregar_do_sqlite(&conn).unwrap();
        let alertas = grafo.detectar_fornecedores_hub();

        assert_eq!(alertas.len(), 1);
        let a = &alertas[0];
        assert_eq!(a.fornecedor_id, emp_id);
        assert_eq!(a.percentual_concentracao, 90.0);
        assert_eq!(a.maior_coligacao_grupo, "COLIGACAO PRA FRENTE");
        assert_eq!(a.quantidade_agentes_conectados, 4);
    }

    #[test]
    fn test_hub_caso_controle_diversificado_sem_alerta() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('emp-div', 'EMPRESA', 'POSTO COMUM')",
            [],
        ).unwrap();
        let emp_id = conn.last_insert_rowid();

        // 3 candidatos de grupos diferentes com repasses equilibrados
        let meta1 = serde_json::json!({ "coligacao": "GRUPO 1" }).to_string();
        let meta2 = serde_json::json!({ "coligacao": "GRUPO 2" }).to_string();
        let meta3 = serde_json::json!({ "coligacao": "GRUPO 3" }).to_string();

        conn.execute("INSERT INTO nos_rede (uuid, tipo, nome, metadata_json) VALUES ('g1', 'POLITICO', 'CAND 1', ?1)", [&meta1]).unwrap();
        let g1 = conn.last_insert_rowid();
        conn.execute("INSERT INTO nos_rede (uuid, tipo, nome, metadata_json) VALUES ('g2', 'POLITICO', 'CAND 2', ?1)", [&meta2]).unwrap();
        let g2 = conn.last_insert_rowid();
        conn.execute("INSERT INTO nos_rede (uuid, tipo, nome, metadata_json) VALUES ('g3', 'POLITICO', 'CAND 3', ?1)", [&meta3]).unwrap();
        let g3 = conn.last_insert_rowid();

        // Cada um repassa 33.3%
        conn.execute("INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado) VALUES (?1, ?2, 'DESPESA', 1000.0, 2024, 'TSE')", storage::rusqlite::params![g1, emp_id]).unwrap();
        conn.execute("INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado) VALUES (?1, ?2, 'DESPESA', 1000.0, 2024, 'TSE')", storage::rusqlite::params![g2, emp_id]).unwrap();
        conn.execute("INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado) VALUES (?1, ?2, 'DESPESA', 1000.0, 2024, 'TSE')", storage::rusqlite::params![g3, emp_id]).unwrap();

        let grafo = GrafoSincronizado::carregar_do_sqlite(&conn).unwrap();
        let alertas = grafo.detectar_fornecedores_hub();

        assert!(alertas.is_empty());
    }
}
