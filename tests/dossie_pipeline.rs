use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use server::criar_router;
use server::dossie::{DossieCnpjResponse, DossieCpfResponse};
use storage::{run_migrations, DbPool};
use tower::ServiceExt;

#[tokio::test]
async fn test_pipeline_completo_dossie_analitico_cnpj_e_cpf() {
    let pool = DbPool::open_in_memory().expect("falha ao abrir sqlite");
    {
        let mut conn = pool.get().expect("falha conexao");
        run_migrations(&mut conn).expect("falha migracoes");

        // 1. Inserir Político e Candidatura
        conn.execute(
            "INSERT INTO politicos (id, sq_candidato, cpf_mascarado, nome_completo, nome_urna)
             VALUES (1, 'SQ9999', '***.111.222-**', 'DEPUTADO JOAO SANTOS', 'JOAO SANTOS')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO candidaturas (id, politico_id, ano_eleicao, cargo, numero_urna, sigla_partido, uf, total_bens_declarados)
             VALUES (10, 1, 2022, 'DEPUTADO FEDERAL', 1010, 'PARTIDO BRASIL', 'SP', 500000.0)",
            [],
        )
        .unwrap();

        // 2. Inserir Empresa no QSA da Receita Federal
        conn.execute(
            "INSERT INTO empresas_qsa (cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_cpf_cnpj_mascarado, socio_nome, qualificacao_socio)
             VALUES ('27403781', '0001', '11', 'BAL MAQ ASSISTENCIA TECNICA DE MAQUINAS LTDA', '***052857**', 'CARLOS SILVA', '49 - SÓCIO-ADMINISTRADOR')",
            [],
        )
        .unwrap();

        // 3. Inserir Outra Empresa vinculada ao mesmo sócio
        conn.execute(
            "INSERT INTO empresas_qsa (cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_cpf_cnpj_mascarado, socio_nome, qualificacao_socio)
             VALUES ('12345678', '0001', '99', 'SILVA HOLDINGS PARTICIPACOES LTDA', '***052857**', 'CARLOS SILVA', 'SÓCIO')",
            [],
        )
        .unwrap();

        // 4. Inserir Doação de Campanha no TSE feita pelo sócio para o candidato
        conn.execute(
            "INSERT INTO receitas_campanha (candidatura_id, doador_cpf_cnpj, doador_nome, valor, data_receita, tipo_origem)
             VALUES (10, '***052857**', 'CARLOS SILVA', 10000.0, '2022-09-15', 'TRANSFERENCIA')",
            [],
        )
        .unwrap();

        // 5. Inserir Despesa CEAP (Câmara) onde o mesmo político contrata a empresa do sócio
        conn.execute(
            "INSERT INTO despesas_parlamentares (casa_legislativa, parlamentar_nome, data_emissao, categoria_despesa, fornecedor_nome, fornecedor_cnpj_cpf, valor_liquido, numero_documento, flag_anomalia)
             VALUES ('CAMARA', 'DEPUTADO JOAO SANTOS', '2023-04-10', 'MANUTENCAO DE EQUIPAMENTOS', 'BAL MAQ ASSISTENCIA', '27403781000111', 50000.0, 'NF-888', 0)",
            [],
        )
        .unwrap();

        // 6. Inserir Contrato Público no PNCP
        conn.execute(
            "INSERT INTO contratos_publicos (orgao_contratante, fornecedor_cnpj, valor_contratado, objeto, data_assinatura, data_termino)
             VALUES ('MINISTERIO DA EDUCACAO', '27403781000111', 1500000.0, 'FORNECIMENTO DE EQUIPAMENTOS', '2023-01-20', '2024-01-20')",
            [],
        )
        .unwrap();
    }

    let app = criar_router(pool.clone());

    // ------------------------------------------------------------------------
    // Teste 1: Dossiê de CNPJ via GET /api/v1/dossie/cnpj/27403781000111
    // ------------------------------------------------------------------------
    let req_cnpj = Request::builder()
        .uri("/api/v1/dossie/cnpj/27403781000111")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res_cnpj = app.clone().oneshot(req_cnpj).await.unwrap();
    assert_eq!(res_cnpj.status(), StatusCode::OK);

    let bytes_cnpj = to_bytes(res_cnpj.into_body(), usize::MAX).await.unwrap();
    let data_cnpj: DossieCnpjResponse = serde_json::from_slice(&bytes_cnpj).unwrap();

    assert_eq!(data_cnpj.cnpj, "27403781000111");
    assert_eq!(data_cnpj.razao_social, "BAL MAQ ASSISTENCIA TECNICA DE MAQUINAS LTDA");
    assert_eq!(data_cnpj.qsa.total_socios, 1);
    assert_eq!(data_cnpj.qsa.socios[0].nome, "CARLOS SILVA");
    assert_eq!(data_cnpj.qsa.empresas_interligadas.len(), 1);
    assert_eq!(data_cnpj.qsa.empresas_interligadas[0].razao_social, "SILVA HOLDINGS PARTICIPACOES LTDA");

    // Validação CEAP
    assert_eq!(data_cnpj.ceap.total_faturado, 50000.0);
    assert_eq!(data_cnpj.ceap.total_notas, 1);
    assert_eq!(data_cnpj.ceap.compradores.len(), 1);
    assert_eq!(data_cnpj.ceap.compradores[0].parlamentar_nome, "DEPUTADO JOAO SANTOS");

    // Validação PNCP
    assert_eq!(data_cnpj.pncp.total_contratado, 1500000.0);
    assert_eq!(data_cnpj.pncp.total_contratos, 1);
    assert_eq!(data_cnpj.pncp.orgaos_contratantes[0].orgao, "MINISTERIO DA EDUCACAO");

    // Validação Heurística TSE
    assert_eq!(data_cnpj.tse.total_doacoes_socios, 10000.0);
    assert_eq!(data_cnpj.tse.doacoes[0].candidato_nome, "DEPUTADO JOAO SANTOS");

    // Validação de Triangulação Detectada (Sócio doou para deputado que contratou a empresa)
    let triangulacao = data_cnpj
        .alertas
        .iter()
        .find(|a| a.tipo == "TRIANGULACAO_HUB");
    assert!(triangulacao.is_some(), "Alerta de triangulação eleitoral deve ser gerado");
    assert_eq!(data_cnpj.score_risco, "CRITICO");

    // ------------------------------------------------------------------------
    // Teste 2: Dossiê com CNPJ formatado 27.403.781/0001-11
    // ------------------------------------------------------------------------
    let req_cnpj_fmt = Request::builder()
        .uri("/api/v1/dossie/cnpj/27.403.781%2F0001-11")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res_cnpj_fmt = app.clone().oneshot(req_cnpj_fmt).await.unwrap();
    assert_eq!(res_cnpj_fmt.status(), StatusCode::OK);

    // ------------------------------------------------------------------------
    // Teste 3: Dossiê de CPF via GET /api/v1/dossie/cpf/***052857**
    // ------------------------------------------------------------------------
    let req_cpf = Request::builder()
        .uri("/api/v1/dossie/cpf/%2A%2A%2A052857%2A%2A")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res_cpf = app.clone().oneshot(req_cpf).await.unwrap();
    assert_eq!(res_cpf.status(), StatusCode::OK);

    let bytes_cpf = to_bytes(res_cpf.into_body(), usize::MAX).await.unwrap();
    let data_cpf: DossieCpfResponse = serde_json::from_slice(&bytes_cpf).unwrap();

    assert_eq!(data_cpf.nome, "CARLOS SILVA");
    assert_eq!(data_cpf.empresas_socio.len(), 2);
    assert_eq!(data_cpf.doacoes_eleitorais.len(), 1);
    assert_eq!(data_cpf.doacoes_eleitorais[0].valor, 10000.0);
    assert_eq!(data_cpf.total_faturado_empresas_ceap, 50000.0);
    assert_eq!(data_cpf.total_contratado_empresas_pncp, 1500000.0);
    assert_eq!(data_cpf.score_risco, "CRITICO");
}
