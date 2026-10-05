use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
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

    pub fn node_count(&self) -> usize {
        self.grafo.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.grafo.edge_count()
    }

    pub fn obter_no_por_id(&self, db_id: i64) -> Option<&NoRede> {
        self.id_map.get(&db_id).and_then(|&idx| self.grafo.node_weight(idx))
    }

    pub fn obter_no_por_uuid(&self, uuid: &str) -> Option<&NoRede> {
        self.uuid_map.get(uuid).and_then(|&idx| self.grafo.node_weight(idx))
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
        ).unwrap();
        let id1 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, documento, nome, esfera, uf)
             VALUES ('emp-1', 'EMPRESA', '222', 'EMPRESA AMIGA', 'MUNICIPAL', 'SP')",
            [],
        ).unwrap();
        let id2 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, documento, nome, esfera, uf)
             VALUES ('doa-1', 'PESSOA_FISICA', '333', 'DOADOR SILVA', 'MUNICIPAL', 'SP')",
            [],
        ).unwrap();
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
