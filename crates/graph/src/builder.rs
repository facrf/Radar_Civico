use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use storage::rusqlite::Connection;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoRede {
    pub db_id: i64,
    pub uuid: String,
    pub tipo: String,
    pub documento: Option<String>,
    pub nome: String,
    pub esfera: Option<String>,
    pub uf: Option<String>,
    pub municipio: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArestaRede {
    pub db_id: i64,
    pub origem_id: i64,
    pub destino_id: i64,
    pub tipo_relacao: String,
    pub valor: f64,
    pub ano: i32,
    pub data_evento: Option<String>,
    pub fonte_dado: String,
    pub metadata_json: Option<String>,
}

pub type RedeGrafo = DiGraph<NoRede, ArestaRede>;

#[derive(Clone)]
pub struct GrafoSincronizado {
    pub grafo: RedeGrafo,
    pub id_map: HashMap<i64, NodeIndex>,
    pub uuid_map: HashMap<String, NodeIndex>,
}

impl GrafoSincronizado {
    pub fn carregar_do_sqlite(conn: &Connection) -> Result<Self> {
        let mut grafo = DiGraph::<NoRede, ArestaRede>::new();
        let mut id_map = HashMap::new();
        let mut uuid_map = HashMap::new();

        // 1. Carrega todos os nós
        {
            let mut stmt = conn.prepare(
                "SELECT id, uuid, tipo, documento, nome, esfera, uf, municipio, metadata_json
                 FROM nos_rede
                 ORDER BY id ASC",
            )?;

            let rows = stmt.query_map([], |row| {
                Ok(NoRede {
                    db_id: row.get(0)?,
                    uuid: row.get(1)?,
                    tipo: row.get(2)?,
                    documento: row.get(3)?,
                    nome: row.get(4)?,
                    esfera: row.get(5)?,
                    uf: row.get(6)?,
                    municipio: row.get(7)?,
                    metadata_json: row.get(8)?,
                })
            })?;

            for no_res in rows {
                let no = no_res?;
                let db_id = no.db_id;
                let uuid = no.uuid.clone();
                let idx = grafo.add_node(no);

                id_map.insert(db_id, idx);
                uuid_map.insert(uuid, idx);
            }
        }

        // 2. Carrega todas as arestas
        {
            let mut stmt = conn.prepare(
                "SELECT id, origem_id, destino_id, tipo_relacao, valor, ano, data_evento, fonte_dado, metadata_json
                 FROM conexoes_rede
                 ORDER BY id ASC",
            )?;

            let rows = stmt.query_map([], |row| {
                Ok(ArestaRede {
                    db_id: row.get(0)?,
                    origem_id: row.get(1)?,
                    destino_id: row.get(2)?,
                    tipo_relacao: row.get(3)?,
                    valor: row.get(4)?,
                    ano: row.get(5)?,
                    data_evento: row.get(6)?,
                    fonte_dado: row.get(7)?,
                    metadata_json: row.get(8)?,
                })
            })?;

            for aresta_res in rows {
                let aresta = aresta_res?;
                if let (Some(&origem_idx), Some(&destino_idx)) = (
                    id_map.get(&aresta.origem_id),
                    id_map.get(&aresta.destino_id),
                ) {
                    grafo.add_edge(origem_idx, destino_idx, aresta);
                }
            }
        }

        Ok(Self {
            grafo,
            id_map,
            uuid_map,
        })
    }

    /// Carrega apenas a vizinhança não direcionada e suas arestas internas.
    /// Limites explícitos impedem que hubs esgotem a memória da API.
    pub fn carregar_vizinhanca(conn: &Connection, root: &str, grau: usize) -> Result<Self> {
        use std::collections::{HashSet, VecDeque};
        use storage::rusqlite::OptionalExtension;
        let root_id: Option<i64> = conn
            .query_row("SELECT id FROM nos_rede WHERE uuid=?1", [root], |r| {
                r.get(0)
            })
            .optional()?;
        let root_id = match root_id {
            Some(id) => Some(id),
            None => match root.parse::<i64>() {
                Ok(id) => conn
                    .query_row("SELECT id FROM nos_rede WHERE id=?1", [id], |r| r.get(0))
                    .optional()?,
                Err(_) => None,
            },
        }
        .ok_or_else(|| crate::error::GraphError::NodeNotFound(root.into()))?;
        let mut ids = HashSet::from([root_id]);
        let mut queue = VecDeque::from([(root_id, 0)]);
        let mut neighbors = conn.prepare("SELECT destino_id FROM conexoes_rede WHERE origem_id=?1 UNION SELECT origem_id FROM conexoes_rede WHERE destino_id=?1 LIMIT 10001")?;
        while let Some((id, depth)) = queue.pop_front() {
            if depth >= grau.min(5) {
                continue;
            }
            for next in neighbors.query_map([id], |r| r.get::<_, i64>(0))? {
                let next = next?;
                if ids.insert(next) {
                    if ids.len() > 10000 {
                        return Err(crate::error::GraphError::TooLarge);
                    }
                    queue.push_back((next, depth + 1));
                }
            }
        }
        conn.execute_batch("CREATE TEMP TABLE IF NOT EXISTS grafo_ids(id INTEGER PRIMARY KEY); DELETE FROM grafo_ids;")?;
        let mut insert = conn.prepare("INSERT INTO temp.grafo_ids(id) VALUES (?1)")?;
        for id in ids {
            insert.execute([id])?;
        }
        let mut graph = Self {
            grafo: DiGraph::new(),
            id_map: HashMap::new(),
            uuid_map: HashMap::new(),
        };
        let mut nodes=conn.prepare("SELECT n.id,n.uuid,n.tipo,n.documento,n.nome,n.esfera,n.uf,n.municipio,n.metadata_json FROM temp.grafo_ids g JOIN nos_rede n ON n.id=g.id ORDER BY n.id")?;
        let rows = nodes.query_map([], |r| {
            Ok(NoRede {
                db_id: r.get(0)?,
                uuid: r.get(1)?,
                tipo: r.get(2)?,
                documento: r.get(3)?,
                nome: r.get(4)?,
                esfera: r.get(5)?,
                uf: r.get(6)?,
                municipio: r.get(7)?,
                metadata_json: r.get(8)?,
            })
        })?;
        for node in rows {
            let node = node?;
            let id = node.db_id;
            let uuid = node.uuid.clone();
            let index = graph.grafo.add_node(node);
            graph.id_map.insert(id, index);
            graph.uuid_map.insert(uuid, index);
        }
        let mut edges=conn.prepare("SELECT e.id,e.origem_id,e.destino_id,e.tipo_relacao,e.valor_centavos/100.0,e.ano,e.data_evento,e.fonte_dado,e.metadata_json FROM temp.grafo_ids g CROSS JOIN conexoes_rede e ON e.origem_id=g.id JOIN temp.grafo_ids d ON d.id=e.destino_id LIMIT 50001")?;
        let rows = edges.query_map([], |r| {
            Ok(ArestaRede {
                db_id: r.get(0)?,
                origem_id: r.get(1)?,
                destino_id: r.get(2)?,
                tipo_relacao: r.get(3)?,
                valor: r.get(4)?,
                ano: r.get(5)?,
                data_evento: r.get(6)?,
                fonte_dado: r.get(7)?,
                metadata_json: r.get(8)?,
            })
        })?;
        for edge in rows {
            let edge = edge?;
            if graph.edge_count() >= 50000 {
                return Err(crate::error::GraphError::TooLarge);
            }
            graph.grafo.add_edge(
                graph.id_map[&edge.origem_id],
                graph.id_map[&edge.destino_id],
                edge,
            );
        }
        conn.execute("DELETE FROM temp.grafo_ids", [])?;
        Ok(graph)
    }

    pub fn node_count(&self) -> usize {
        self.grafo.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.grafo.edge_count()
    }

    pub fn obter_no_por_id(&self, db_id: i64) -> Option<&NoRede> {
        self.id_map
            .get(&db_id)
            .and_then(|&idx| self.grafo.node_weight(idx))
    }

    pub fn obter_no_por_uuid(&self, uuid: &str) -> Option<&NoRede> {
        self.uuid_map
            .get(uuid)
            .and_then(|&idx| self.grafo.node_weight(idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::{run_migrations, DbPool};

    #[test]
    fn test_builder_sincroniza_grafo_sqlite() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // Insere nós de teste
        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, documento, nome, esfera, uf)
             VALUES ('pol-1', 'POLITICO', '111', 'PREFEITO ALVO', 'MUNICIPAL', 'SP')",
            [],
        )
        .unwrap();
        let id1 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, documento, nome, esfera, uf)
             VALUES ('emp-1', 'EMPRESA', '222', 'EMPRESA AMIGA', 'MUNICIPAL', 'SP')",
            [],
        )
        .unwrap();
        let id2 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, documento, nome, esfera, uf)
             VALUES ('doa-1', 'PESSOA_FISICA', '333', 'DOADOR SILVA', 'MUNICIPAL', 'SP')",
            [],
        )
        .unwrap();
        let id3 = conn.last_insert_rowid();

        // Insere conexões: doador -> politico (doação) e empresa -> politico (contrato)
        conn.execute(
            "INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado)
             VALUES (?1, ?2, 'DOACAO_CAMPANHA', 25000.0, 2024, 'TSE')",
            storage::rusqlite::params![id3, id1],
        ).unwrap();

        conn.execute(
            "INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado)
             VALUES (?1, ?2, 'CONTRATO_PUBLICO', 500000.0, 2024, 'PNCP')",
            storage::rusqlite::params![id2, id1],
        ).unwrap();

        let grafo_sync = GrafoSincronizado::carregar_do_sqlite(&conn).unwrap();

        assert_eq!(grafo_sync.node_count(), 3);
        assert_eq!(grafo_sync.edge_count(), 2);

        let no_pol = grafo_sync.obter_no_por_uuid("pol-1").unwrap();
        assert_eq!(no_pol.nome, "PREFEITO ALVO");
        assert_eq!(no_pol.tipo, "POLITICO");

        let no_emp = grafo_sync.obter_no_por_id(id2).unwrap();
        assert_eq!(no_emp.nome, "EMPRESA AMIGA");
    }
}

#[cfg(test)]
mod neighborhood_regressions {
    use super::*;
    #[test]
    fn vizinhanca_equivale_ao_grafo_completo_e_exclui_componentes() {
        let pool = storage::DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        storage::run_migrations(&mut conn).unwrap();
        for id in 1..=5 {
            conn.execute(
                "INSERT INTO nos_rede(id,uuid,tipo,nome) VALUES (?1,?2,'EMPRESA','Empresa')",
                storage::rusqlite::params![id, format!("n{id}")],
            )
            .unwrap();
        }
        for (a, b) in [(1, 2), (3, 2), (3, 4), (1, 1)] {
            conn.execute("INSERT INTO conexoes_rede(origem_id,destino_id,tipo_relacao,ano,fonte_dado) VALUES (?1,?2,'SOCIO',2024,'QSA')",[a,b]).unwrap();
        }
        let full = GrafoSincronizado::carregar_do_sqlite(&conn).unwrap();
        for degree in 0..=3 {
            let local = GrafoSincronizado::carregar_vizinhanca(&conn, "n1", degree).unwrap();
            let expected = full.extrair_subgrafo_vizinhanca("n1", degree).unwrap();
            assert_eq!(local.node_count(), expected.nos.len());
            assert_eq!(local.edge_count(), expected.arestas.len());
            assert!(local.obter_no_por_id(5).is_none());
        }
        assert!(matches!(
            GrafoSincronizado::carregar_vizinhanca(&conn, "absent", 2),
            Err(crate::GraphError::NodeNotFound(_))
        ));
        assert_eq!(
            GrafoSincronizado::carregar_vizinhanca(&conn, "1", 1)
                .unwrap()
                .node_count(),
            2
        );
    }
}
