use axum::body::Body;
use axum::http::{Request, StatusCode};
use graph::GrafoSincronizado;
use ingestion::{
    mascarar_cpf, ExcerptDiario, OabConsultaResult, OabScraperClient,
    QueridoDiarioApiResponse, QueridoDiarioClient,
};
use server::alertas::AlertasResponse;
use server::grafo::SubgrafoResponse;
use server::investigar::{executar_investigacao, identificar_doador, InvestigarParams};
use server::politico::DossiePolitico;
use server::criar_router;
use storage::rusqlite::params;
use storage::{run_migrations, DbPool};
use tower::ServiceExt;

#[tokio::test]
async fn test_pipeline_completo_ingestao_auditoria_e_grafo() {
    // 1. Inicializa banco SQLite in-memory de alta performance com migrações
    let pool = DbPool::open_in_memory().expect("falha ao inicializar banco in-memory");
    let mut conn = pool.get().expect("falha ao obter conexão do pool");
    run_migrations(&mut conn).expect("falha ao aplicar migrações");

    // 2. Ingestão simulada: Candidato a Prefeito com Bens declarados
    let foto_fake: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46]; // JPEG header
    conn.execute(
        "INSERT INTO politicos (sq_candidato, cpf_mascarado, nome_completo, nome_urna, ocupacao, grau_instrucao, foto_blob, foto_mime)
         VALUES ('250001234567', '***.456.789-**', 'JOAO DA SILVA SAO PAULO', 'JOAO PREFEITO', 'ADMINISTRADOR', 'SUPERIOR COMPLETO', ?1, 'image/jpeg')",
        params![foto_fake],
    ).expect("falha ao inserir politico");
    let politico_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, numero_urna, sigla_partido, uf, municipio, situacao_totalizacao)
         VALUES (?1, 2024, 'PREFEITO', 45, 'PARTIDO PROGRESSO', 'SP', 'Campinas', 'ELEITO')",
        params![politico_id],
    ).expect("falha ao inserir candidatura");
    let candidatura_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO bens_candidato (candidatura_id, tipo_bem, descricao, valor_declarado)
         VALUES (?1, 'IMOVEL', 'Apartamento residencial', 750000.00)",
        params![candidatura_id],
    ).expect("falha ao inserir bem");

    // 3. Ingestão simulada: Receita de Campanha oriunda de Doador Advogado
    let doc_advogado = "12345678901";
    let doc_mascarado = mascarar_cpf(doc_advogado);
    let nome_advogado = "DR. ANTONIO CARLOS ADVOGADO";

    conn.execute(
        "INSERT INTO receitas_campanha (candidatura_id, doador_cpf_cnpj, doador_nome, valor, data_receita, tipo_origem)
         VALUES (?1, ?2, ?3, 30000.00, '2024-09-10', 'TRANSFERENCIA_ELETRONICA')",
        params![candidatura_id, doc_advogado, nome_advogado],
    ).expect("falha ao inserir receita");
    let receita_id = conn.last_insert_rowid();

    // 4. Ingestão simulada no Grafo de Relacionamentos (nos_rede e conexoes_rede)
    conn.execute(
        "INSERT INTO nos_rede (uuid, tipo, documento, nome, esfera, uf, municipio)
         VALUES ('cand-45', 'POLITICO', '***.456.789-**', 'JOAO PREFEITO', 'MUNICIPAL', 'SP', 'Campinas')",
        [],
    ).expect("falha ao inserir no do politico");
    let no_politico_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO nos_rede (uuid, tipo, documento, nome, esfera, uf, municipio)
         VALUES ('doador-10', 'PESSOA_FISICA', ?1, ?2, 'MUNICIPAL', 'SP', 'Campinas')",
        params![doc_advogado, nome_advogado],
    ).expect("falha ao inserir no do doador");
    let no_doador_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado)
         VALUES (?1, ?2, 'DOACAO_CAMPANHA', 30000.00, 2024, 'TSE')",
        params![no_doador_id, no_politico_id],
    ).expect("falha ao inserir conexao doacao");

    // 5. Ingestão simulada: Cache do Querido Diário comprovando nomeação como Secretário Municipal
    let trecho_nomeacao = format!(
        "PREFEITURA MUNICIPAL DE CAMPINAS. DECRETO Nº 21.000: O PREFEITO MUNICIPAL, NO USO DE SUAS ATRIBUIÇÕES LEGAIS, RESOLVE: NOMEAR {} PARA EXERCER O CARGO DE SECRETÁRIO MUNICIPAL DE ASSUNTOS JURÍDICOS.",
        nome_advogado
    );
    let gazette_resp = QueridoDiarioApiResponse {
        total_gazettes: 1,
        gazettes: vec![ExcerptDiario {
            date: Some("2025-01-02".to_string()),
            territory_name: Some("Campinas".to_string()),
            territory_id: Some("3509502".to_string()),
            url: Some("https://diario.campinas.sp.gov.br/atos/2025/decreto-21000.pdf".to_string()),
            excerpt: trecho_nomeacao,
        }],
    };
    QueridoDiarioClient::salvar_cache(
        &mut conn,
        doc_advogado,
        nome_advogado,
        "Campinas-SP",
        1,
        &serde_json::to_string(&gazette_resp).unwrap(),
    ).expect("falha ao salvar cache do querido diario");

    // 6. Ingestão simulada: Registro Profissional no CNA/OAB com situação "REGULAR" (Incompatível!)
    let oab_rec = OabConsultaResult {
        nome: nome_advogado.to_string(),
        inscricao: "345678".to_string(),
        uf: "SP".to_string(),
        situacao: "REGULAR".to_string(),
        tipo: "ADVOGADO".to_string(),
        payload_json: "{\"Situacao\":\"REGULAR\"}".to_string(),
    };
    OabScraperClient::salvar_registro_oab(&mut conn, &oab_rec, Some(&doc_mascarado))
        .expect("falha ao salvar registro oab");

    // 7. Execução do Motor de Auditoria / Investigação Sob Demanda
    let doador_identificado = identificar_doador(&conn, &receita_id.to_string())
        .expect("doador não identificado a partir da receita");
    assert_eq!(doador_identificado.nome, nome_advogado);

    let investigacao_res = executar_investigacao(
        &mut conn,
        doador_identificado,
        InvestigarParams {
            municipio: Some("Campinas".to_string()),
            uf: Some("SP".to_string()),
            forcar: Some(false),
        },
    ).await.expect("falha ao executar investigacao");

    // Valida detecção do conflito de interesses (Art. 28 da Lei 8.906/94)
    assert_eq!(investigacao_res.status, "ALERTA_GERADO");
    assert!(investigacao_res.alerta_conflito.is_some());
    let alerta_oab = investigacao_res.alerta_conflito.unwrap();
    assert_eq!(alerta_oab.oab_registro, "345678");
    assert_eq!(alerta_oab.situacao_oab, "REGULAR");
    assert!(alerta_oab.cargo.contains("SECRETÁRIO MUNICIPAL"));

    // 8. Validação do Grafo em Memória
    let grafo = GrafoSincronizado::carregar_do_sqlite(&conn)
        .expect("falha ao sincronizar grafo");
    let subgrafo = grafo.extrair_subgrafo_vizinhanca("cand-45", 1)
        .expect("subgrafo não encontrado");
    assert_eq!(subgrafo.nos.len(), 2);
    assert_eq!(subgrafo.arestas.len(), 1);
    assert_eq!(subgrafo.arestas[0].valor, 30000.00);

    // 9. Validação dos Endpoints HTTP Axum via Router Completo
    let app = criar_router(pool);

    // Teste 9.1: Endpoint de Dossiê Consolidado
    let req_dossie = Request::builder()
        .uri(format!("/api/v1/politico/{}", politico_id))
        .body(Body::empty())
        .unwrap();
    let resp_dossie = app.clone().oneshot(req_dossie).await.unwrap();
    assert_eq!(resp_dossie.status(), StatusCode::OK);
    let bytes_dossie = axum::body::to_bytes(resp_dossie.into_body(), 1024 * 1024).await.unwrap();
    let dossie_json: DossiePolitico = serde_json::from_slice(&bytes_dossie).unwrap();
    assert_eq!(dossie_json.nome_urna, "JOAO PREFEITO");
    assert_eq!(dossie_json.historico_bens.len(), 1);
    assert_eq!(dossie_json.historico_bens[0].valor_declarado, 750000.00);
    assert_eq!(dossie_json.doadores.len(), 1);
    assert_eq!(dossie_json.doadores[0].valor, 30000.00);
    assert!(dossie_json.foto_base64.is_some());

    // Teste 9.2: Endpoint de Alertas do Auditor
    let req_alertas = Request::builder()
        .uri("/api/v1/auditoria/alertas?tipo=CONFLITO_OAB")
        .body(Body::empty())
        .unwrap();
    let resp_alertas = app.clone().oneshot(req_alertas).await.unwrap();
    assert_eq!(resp_alertas.status(), StatusCode::OK);
    let bytes_alertas = axum::body::to_bytes(resp_alertas.into_body(), 1024 * 1024).await.unwrap();
    let alertas_json: AlertasResponse = serde_json::from_slice(&bytes_alertas).unwrap();
    assert!(alertas_json.total >= 1);
    assert_eq!(alertas_json.alertas[0].severidade, "CRITICA");
    assert_eq!(alertas_json.alertas[0].alvo_nome, nome_advogado);

    // Teste 9.3: Endpoint de Subgrafo formatado para Cytoscape / ECharts
    let req_grafo = Request::builder()
        .uri("/api/v1/grafo/cand-45?grau=1")
        .body(Body::empty())
        .unwrap();
    let resp_grafo = app.oneshot(req_grafo).await.unwrap();
    assert_eq!(resp_grafo.status(), StatusCode::OK);
    let bytes_grafo = axum::body::to_bytes(resp_grafo.into_body(), 1024 * 1024).await.unwrap();
    let grafo_json: SubgrafoResponse = serde_json::from_slice(&bytes_grafo).unwrap();
    assert_eq!(grafo_json.raiz_id, "cand-45");
    assert_eq!(grafo_json.total_nos, 2);
    assert_eq!(grafo_json.cytoscape.nodes.len(), 2);
    assert_eq!(grafo_json.cytoscape.edges.len(), 1);
    assert_eq!(grafo_json.echarts.nodes.len(), 2);
    assert_eq!(grafo_json.echarts.links.len(), 1);
}
