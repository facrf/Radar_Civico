use std::collections::{HashMap, HashSet};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use graph::{ArestaRede, GrafoSincronizado, NoRede};
use serde::{Deserialize, Serialize};
use storage::DbPool;

#[derive(Debug, Deserialize)]
pub struct GrafoParams {
    pub grau: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CytoscapeNodeData {
    pub id: String,
    pub label: String,
    pub tipo: String,
    pub documento: Option<String>,
    pub esfera: Option<String>,
    pub uf: Option<String>,
    pub municipio: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CytoscapeNode {
    pub data: CytoscapeNodeData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CytoscapeEdgeData {
    pub id: String,
    pub source: String,
    pub target: String,
    pub tipo_relacao: String,
    pub valor: f64,
    pub ano: i32,
    pub fonte_dado: String,
    pub anomalia: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CytoscapeEdge {
    pub data: CytoscapeEdgeData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CytoscapeElements {
    pub nodes: Vec<CytoscapeNode>,
    pub edges: Vec<CytoscapeEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EchartsNode {
    pub id: String,
    pub name: String,
    pub category: String,
    pub value: f64,
    pub symbol_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EchartsLink {
    pub source: String,
    pub target: String,
    pub value: f64,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EchartsCategory {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EchartsGraph {
    pub nodes: Vec<EchartsNode>,
    pub links: Vec<EchartsLink>,
    pub categories: Vec<EchartsCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubgrafoResponse {
    pub raiz_id: String,
    pub grau: usize,
    pub total_nos: usize,
    pub total_arestas: usize,
    pub nos: Vec<NoRede>,
    pub arestas: Vec<ArestaRede>,
    pub cytoscape: CytoscapeElements,
    pub echarts: EchartsGraph,
}

pub fn formatar_subgrafo(
    grafo: &GrafoSincronizado,
    raiz_uuid_ou_id: &str,
    grau: usize,
) -> Option<SubgrafoResponse> {
    let sub = grafo.extrair_subgrafo_vizinhanca(raiz_uuid_ou_id, grau)?;

    // Mapeamento db_id -> uuid
    let mut id_to_uuid = HashMap::new();
    for n in &sub.nos {
        id_to_uuid.insert(n.db_id, n.uuid.clone());
    }

    // Categorias únicas para ECharts
    let mut categorias_set = HashSet::new();
    for n in &sub.nos {
        categorias_set.insert(n.tipo.clone());
    }
    let mut categories_list: Vec<String> = categorias_set.into_iter().collect();
    categories_list.sort();
    let categories: Vec<EchartsCategory> = categories_list
        .iter()
        .map(|c| EchartsCategory { name: c.clone() })
        .collect();

    // Elementos Cytoscape
    let mut cy_nodes = Vec::with_capacity(sub.nos.len());
    let mut echarts_nodes = Vec::with_capacity(sub.nos.len());

    for n in &sub.nos {
        let meta_val: Option<serde_json::Value> = n
            .metadata_json
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok());

        cy_nodes.push(CytoscapeNode {
            data: CytoscapeNodeData {
                id: n.uuid.clone(),
                label: n.nome.clone(),
                tipo: n.tipo.clone(),
                documento: n.documento.clone(),
                esfera: n.esfera.clone(),
                uf: n.uf.clone(),
                municipio: n.municipio.clone(),
                metadata: meta_val,
            },
        });

        let is_root = n.uuid == sub.raiz.uuid;
        let symbol_size = if is_root { 45.0 } else { 25.0 };

        echarts_nodes.push(EchartsNode {
            id: n.uuid.clone(),
            name: n.nome.clone(),
            category: n.tipo.clone(),
            value: 0.0,
            symbol_size,
        });
    }

    let mut cy_edges = Vec::with_capacity(sub.arestas.len());
    let mut echarts_links = Vec::with_capacity(sub.arestas.len());

    for a in &sub.arestas {
        let source_uuid = id_to_uuid
            .get(&a.origem_id)
            .cloned()
            .unwrap_or_else(|| a.origem_id.to_string());
        let target_uuid = id_to_uuid
            .get(&a.destino_id)
            .cloned()
            .unwrap_or_else(|| a.destino_id.to_string());

        let eh_anomalia = a.tipo_relacao.to_uppercase().contains("ANOMALIA")
            || a.tipo_relacao.to_uppercase().contains("SUSPEIT")
            || a.tipo_relacao.to_uppercase().contains("PARENTESCO")
            || a.tipo_relacao.to_uppercase().contains("NEPOTISMO")
            || a.metadata_json
                .as_ref()
                .map(|m| m.contains("anomalia") || m.contains("alerta"))
                .unwrap_or(false);

        cy_edges.push(CytoscapeEdge {
            data: CytoscapeEdgeData {
                id: format!("e-{}", a.db_id),
                source: source_uuid.clone(),
                target: target_uuid.clone(),
                tipo_relacao: a.tipo_relacao.clone(),
                valor: a.valor,
                ano: a.ano,
                fonte_dado: a.fonte_dado.clone(),
                anomalia: eh_anomalia,
            },
        });

        echarts_links.push(EchartsLink {
            source: source_uuid,
            target: target_uuid,
            value: a.valor,
            label: a.tipo_relacao.clone(),
        });
    }

    Some(SubgrafoResponse {
        raiz_id: sub.raiz.uuid.clone(),
        grau,
        total_nos: sub.nos.len(),
        total_arestas: sub.arestas.len(),
        nos: sub.nos,
        arestas: sub.arestas,
        cytoscape: CytoscapeElements {
            nodes: cy_nodes,
            edges: cy_edges,
        },
        echarts: EchartsGraph {
            nodes: echarts_nodes,
            links: echarts_links,
            categories,
        },
    })
}

pub async fn grafo_subgrafo_handler(
    State(pool): State<DbPool>,
    Path(id): Path<String>,
    Query(params): Query<GrafoParams>,
) -> Result<Json<SubgrafoResponse>, StatusCode> {
    let grau = params.grau.unwrap_or(2).min(5);

    let conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let grafo = GrafoSincronizado::carregar_do_sqlite(&conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = formatar_subgrafo(&grafo, &id, grau).ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use storage::run_migrations;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_grafo_subgrafo_endpoint() {
        let pool = DbPool::open_in_memory().unwrap();
        let mut conn = pool.get().unwrap();
        run_migrations(&mut conn).unwrap();

        // Cria nós da rede
        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('pol-100', 'POLITICO', 'FULANO DE TAL')",
            [],
        ).unwrap();
        let n1 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('emp-200', 'EMPRESA', 'POSTO COMBUSTIVEL LTDA')",
            [],
        ).unwrap();
        let n2 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('pes-300', 'PESSOA_FISICA', 'SOCIO EMPRESA')",
            [],
        ).unwrap();
        let n3 = conn.last_insert_rowid();

        // Conexões: pol-100 -> emp-200 (despesa anomalia combustível)
        conn.execute(
            "INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado, metadata_json)
             VALUES (?1, ?2, 'DESPESA_COMBUSTIVEL_ANOMALIA', 350.0, 2024, 'CEAP', '{\"anomalia\":true}')",
            storage::rusqlite::params![n1, n2],
        ).unwrap();

        // emp-200 -> pes-300 (QSA / sociedade)
        conn.execute(
            "INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado)
             VALUES (?1, ?2, 'SOCIO_ADMINISTRADOR', 0.0, 2024, 'RECEITA_FEDERAL')",
            storage::rusqlite::params![n2, n3],
        ).unwrap();

        let app = Router::new()
            .route("/api/v1/grafo/:id", get(grafo_subgrafo_handler))
            .with_state(pool);

        // Teste de consulta com grau=1
        let req1 = Request::builder()
            .uri("/api/v1/grafo/pol-100?grau=1")
            .body(Body::empty())
            .unwrap();

        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::OK);

        let body1 = axum::body::to_bytes(resp1.into_body(), 1024 * 1024).await.unwrap();
        let sub1: SubgrafoResponse = serde_json::from_slice(&body1).unwrap();
        assert_eq!(sub1.raiz_id, "pol-100");
        assert_eq!(sub1.grau, 1);
        assert_eq!(sub1.total_nos, 2); // pol-100 e emp-200
        assert_eq!(sub1.cytoscape.nodes.len(), 2);
        assert_eq!(sub1.cytoscape.edges.len(), 1);
        assert!(sub1.cytoscape.edges[0].data.anomalia); // flag de anomalia ativa
        assert_eq!(sub1.echarts.nodes.len(), 2);

        // Teste de consulta com grau=2 (alcança pes-300 via emp-200)
        let req2 = Request::builder()
            .uri("/api/v1/grafo/pol-100?grau=2")
            .body(Body::empty())
            .unwrap();

        let resp2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);

        let body2 = axum::body::to_bytes(resp2.into_body(), 1024 * 1024).await.unwrap();
        let sub2: SubgrafoResponse = serde_json::from_slice(&body2).unwrap();
        assert_eq!(sub2.grau, 2);
        assert_eq!(sub2.total_nos, 3);
        assert_eq!(sub2.total_arestas, 2);

        // Teste de busca por ID numérico do banco
        let req_num = Request::builder()
            .uri(format!("/api/v1/grafo/{}", n1))
            .body(Body::empty())
            .unwrap();

        let resp_num = app.clone().oneshot(req_num).await.unwrap();
        assert_eq!(resp_num.status(), StatusCode::OK);

        // Teste de nó inexistente -> 404
        let req_404 = Request::builder()
            .uri("/api/v1/grafo/inexistente")
            .body(Body::empty())
            .unwrap();

        let resp_404 = app.oneshot(req_404).await.unwrap();
        assert_eq!(resp_404.status(), StatusCode::NOT_FOUND);
    }
}
