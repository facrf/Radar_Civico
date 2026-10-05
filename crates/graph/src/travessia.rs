use std::collections::HashSet;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};

use crate::builder::{ArestaRede, GrafoSincronizado, NoRede};

pub const GRAU_MAXIMO_PADRAO: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaminhoRede {
    pub nos: Vec<NoRede>,
    pub arestas: Vec<ArestaRede>,
    pub graus: usize,
    pub valor_total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CicloDetectado {
    pub nos: Vec<NoRede>,
    pub arestas: Vec<ArestaRede>,
    pub tamanho: usize,
}

impl GrafoSincronizado {
    pub fn buscar_caminhos_ate_graus(
        &self,
        inicio_uuid: &str,
        destino_uuid: &str,
        max_graus: usize,
    ) -> Vec<CaminhoRede> {
        let inicio_idx = match self.uuid_map.get(inicio_uuid) {
            Some(&idx) => idx,
            None => return Vec::new(),
        };

        let destino_idx = match self.uuid_map.get(destino_uuid) {
            Some(&idx) => idx,
            None => return Vec::new(),
        };

        let mut caminhos_encontrados = Vec::new();
        let mut caminho_atual_nos = vec![inicio_idx];
        let mut caminho_atual_arestas = Vec::new();
        let mut visitados = HashSet::new();
        visitados.insert(inicio_idx);

        self.dfs_caminhos(
            inicio_idx,
            destino_idx,
            max_graus,
            &mut visitados,
            &mut caminho_atual_nos,
            &mut caminho_atual_arestas,
            &mut caminhos_encontrados,
        );

        caminhos_encontrados
    }

    fn dfs_caminhos(
        &self,
        atual: NodeIndex,
        destino: NodeIndex,
        max_graus: usize,
        visitados: &mut HashSet<NodeIndex>,
        caminho_nos: &mut Vec<NodeIndex>,
        caminho_arestas: &mut Vec<ArestaRede>,
        resultados: &mut Vec<CaminhoRede>,
    ) {
        if atual == destino && caminho_arestas.len() > 0 {
            let nos = caminho_nos
                .iter()
                .filter_map(|&idx| self.grafo.node_weight(idx).cloned())
                .collect();
            let valor_total = caminho_arestas.iter().map(|a| a.valor).sum();

            resultados.push(CaminhoRede {
                nos,
                arestas: caminho_arestas.clone(),
                graus: caminho_arestas.len(),
                valor_total,
            });
            return;
        }

        if caminho_arestas.len() >= max_graus {
            return;
        }

        for edge in self.grafo.edges_directed(atual, Direction::Outgoing) {
            let proximo = edge.target();
            if !visitados.contains(&proximo) {
                visitados.insert(proximo);
                caminho_nos.push(proximo);
                caminho_arestas.push(edge.weight().clone());

                self.dfs_caminhos(
                    proximo,
                    destino,
                    max_graus,
                    visitados,
                    caminho_nos,
                    caminho_arestas,
                    resultados,
                );

                caminho_arestas.pop();
                caminho_nos.pop();
                visitados.remove(&proximo);
            }
        }
    }

    pub fn detectar_ciclos_ate_graus(&self, max_graus: usize) -> Vec<CicloDetectado> {
        let mut ciclos = Vec::new();
        let mut ciclo_set = HashSet::new();

        for node_idx in self.grafo.node_indices() {
            let mut caminho_nos = vec![node_idx];
            let mut caminho_arestas = Vec::new();
            let mut visitados = HashSet::new();

            self.dfs_ciclos(
                node_idx,
                node_idx,
                max_graus,
                &mut visitados,
                &mut caminho_nos,
                &mut caminho_arestas,
                &mut ciclos,
                &mut ciclo_set,
            );
        }

        ciclos
    }

    fn dfs_ciclos(
        &self,
        origem: NodeIndex,
        atual: NodeIndex,
        max_graus: usize,
        visitados: &mut HashSet<NodeIndex>,
        caminho_nos: &mut Vec<NodeIndex>,
        caminho_arestas: &mut Vec<ArestaRede>,
        resultados: &mut Vec<CicloDetectado>,
        ciclo_set: &mut HashSet<String>,
    ) {
        if caminho_arestas.len() > max_graus {
            return;
        }

        for edge in self.grafo.edges_directed(atual, Direction::Outgoing) {
            let proximo = edge.target();

            if proximo == origem && caminho_arestas.len() >= 2 {
                // Ciclo encontrado!
                caminho_arestas.push(edge.weight().clone());

                let mut ids: Vec<i64> = caminho_nos
                    .iter()
                    .filter_map(|&idx| self.grafo.node_weight(idx).map(|n| n.db_id))
                    .collect();

                // Normaliza ciclo para evitar duplicação em direções/pontos de entrada diferentes
                let min_id = *ids.iter().min().unwrap_or(&0);
                let pos = ids.iter().position(|&x| x == min_id).unwrap_or(0);
                ids.rotate_left(pos);
                let chave = ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join("->");

                if !ciclo_set.contains(&chave) {
                    ciclo_set.insert(chave);
                    let nos = caminho_nos
                        .iter()
                        .filter_map(|&idx| self.grafo.node_weight(idx).cloned())
                        .collect();

                    resultados.push(CicloDetectado {
                        nos,
                        arestas: caminho_arestas.clone(),
                        tamanho: caminho_arestas.len(),
                    });
                }

                caminho_arestas.pop();
            } else if !visitados.contains(&proximo) && caminho_arestas.len() < max_graus {
                visitados.insert(proximo);
                caminho_nos.push(proximo);
                caminho_arestas.push(edge.weight().clone());

                self.dfs_ciclos(
                    origem,
                    proximo,
                    max_graus,
                    visitados,
                    caminho_nos,
                    caminho_arestas,
                    resultados,
                    ciclo_set,
                );

                caminho_arestas.pop();
                caminho_nos.pop();
                visitados.remove(&proximo);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[test]
    fn test_travessia_caminho_indireto_3_graus() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // 4 Nós: Político P1 -> Doador D -> Empresa E -> Político P2 (3 saltos/graus)
        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('p-1', 'POLITICO', 'POLITICO 1')",
            [],
        ).unwrap();
        let p1 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('d-1', 'PESSOA_FISICA', 'DOADOR INTERMEDIARIO')",
            [],
        ).unwrap();
        let d1 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('e-1', 'EMPRESA', 'EMPRESA BENEFICIADA')",
            [],
        ).unwrap();
        let e1 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('p-2', 'POLITICO', 'POLITICO 2')",
            [],
        ).unwrap();
        let p2 = conn.last_insert_rowid();

        // Conexões: p1 -> d1 (1), d1 -> e1 (2), e1 -> p2 (3)
        conn.execute(
            "INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado)
             VALUES (?1, ?2, 'VINCULO', 100.0, 2024, 'TSE')",
            storage::rusqlite::params![p1, d1],
        ).unwrap();

        conn.execute(
            "INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado)
             VALUES (?1, ?2, 'SOCIEDADE', 200.0, 2024, 'RECEITA')",
            storage::rusqlite::params![d1, e1],
        ).unwrap();

        conn.execute(
            "INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado)
             VALUES (?1, ?2, 'DOACAO', 300.0, 2024, 'TSE')",
            storage::rusqlite::params![e1, p2],
        ).unwrap();

        let grafo = GrafoSincronizado::carregar_do_sqlite(&conn).unwrap();

        // Busca caminhos em até 3 graus
        let caminhos = grafo.buscar_caminhos_ate_graus("p-1", "p-2", 3);
        assert_eq!(caminhos.len(), 1);
        let c = &caminhos[0];
        assert_eq!(c.graus, 3);
        assert_eq!(c.nos.len(), 4);
        assert_eq!(c.valor_total, 600.0);

        // Se limitar a 2 graus, não deve encontrar
        let caminhos_2 = grafo.buscar_caminhos_ate_graus("p-1", "p-2", 2);
        assert!(caminhos_2.is_empty());
    }

    #[test]
    fn test_travessia_detecta_ciclo() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // Ciclo triangular: A -> B -> C -> A
        conn.execute("INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('a', 'POLITICO', 'A')", []).unwrap();
        let a = conn.last_insert_rowid();

        conn.execute("INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('b', 'EMPRESA', 'B')", []).unwrap();
        let b = conn.last_insert_rowid();

        conn.execute("INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('c', 'PESSOA_FISICA', 'C')", []).unwrap();
        let c = conn.last_insert_rowid();

        conn.execute("INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, ano, fonte_dado) VALUES (?1, ?2, 'REL', 2024, 'TSE')", storage::rusqlite::params![a, b]).unwrap();
        conn.execute("INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, ano, fonte_dado) VALUES (?1, ?2, 'REL', 2024, 'TSE')", storage::rusqlite::params![b, c]).unwrap();
        conn.execute("INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, ano, fonte_dado) VALUES (?1, ?2, 'REL', 2024, 'TSE')", storage::rusqlite::params![c, a]).unwrap();

        let grafo = GrafoSincronizado::carregar_do_sqlite(&conn).unwrap();
        let ciclos = grafo.detectar_ciclos_ate_graus(3);

        assert_eq!(ciclos.len(), 1);
        assert_eq!(ciclos[0].tamanho, 3);
    }
}
