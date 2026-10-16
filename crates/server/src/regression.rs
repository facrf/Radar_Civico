//! Regressões do fluxo completo de dados, auditoria, identidade e exportação.
use storage::{rusqlite::params, DbPool};
fn db() -> DbPool {
    let p = DbPool::open_in_memory().unwrap();
    storage::run_migrations(&mut p.get().unwrap()).unwrap();
    p
}
fn politician(conn: &storage::rusqlite::Connection) {
    conn.execute("INSERT INTO politicos(id,sq_candidato,nome_completo,nome_urna,cpf_mascarado) VALUES (42,'SQ42','MARIA','MARIA','12311122207')",[]).unwrap();
    conn.execute("INSERT INTO candidaturas(id,politico_id,ano_eleicao,cargo,sigla_partido,uf) VALUES (42,42,2024,'PREFEITO','ABC','SP')",[]).unwrap();
}
#[test]
fn despesas_nao_multiplicam_por_socios() {
    let p = db();
    let conn = p.get().unwrap();
    for doc in ["12311122207", "98733344400"] {
        conn.execute("INSERT INTO empresas_qsa(cnpj_basico,cnpj_ordem,cnpj_dv,razao_social,socio_cpf_cnpj_mascarado,socio_nome,capital_social) VALUES ('12345678','0001','90','EMPRESA',?1,'SOCIO',1000)",[doc]).unwrap();
    }
    conn.execute("INSERT INTO despesas_parlamentares(casa_legislativa,parlamentar_nome,data_emissao,categoria_despesa,fornecedor_nome,fornecedor_cnpj_cpf,valor_liquido,numero_documento) VALUES ('CAMARA','MARIA','2025-01-01','OUTROS','EMPRESA','12345678000195',60000,'NF1')",[]).unwrap();
    crate::alertas::sincronizar_alertas_sistema(&conn).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM alertas_auditoria WHERE tipo='CAPITAL_DESPROPORCIONAL'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    conn.execute("INSERT INTO despesas_parlamentares(casa_legislativa,parlamentar_nome,data_emissao,categoria_despesa,fornecedor_nome,fornecedor_cnpj_cpf,valor_liquido,numero_documento) VALUES ('CAMARA','MARIA','2025-01-02','OUTROS','EMPRESA','12345678000195',60000,'NF2')",[]).unwrap();
    crate::alertas::sincronizar_alertas_sistema(&conn).unwrap();
    let amount: f64 = conn
        .query_row(
            "SELECT valor_envolvido FROM alertas_auditoria WHERE tipo='CAPITAL_DESPROPORCIONAL'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(amount, 120000.0);
    conn.execute("UPDATE despesas_parlamentares SET valor_liquido=100", [])
        .unwrap();
    crate::alertas::sincronizar_alertas_sistema(&conn).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM alertas_auditoria WHERE tipo='CAPITAL_DESPROPORCIONAL'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn projeção_preserva_ids_e_valores_sem_criar_relacoes_por_mascara() {
    let p = db();
    let conn = p.get().unwrap();
    politician(&conn);
    conn.execute("INSERT INTO receitas_campanha(candidatura_id,doador_cpf_cnpj,doador_nome,valor,data_receita) VALUES (42,'98733344400','DOADOR',123.45,'2024-09-01')",[]).unwrap();
    for (root, name) in [("12345678", "JOAO"), ("87654321", "OUTRA PESSOA")] {
        conn.execute("INSERT INTO empresas_qsa(cnpj_basico,cnpj_ordem,cnpj_dv,razao_social,socio_cpf_cnpj_mascarado,socio_nome) VALUES (?1,'0001','90','EMPRESA','***111222**',?2)",params![root,name]).unwrap();
    }
    let count = crate::projection::refresh(&conn).unwrap();
    assert_eq!(count, 1);
    crate::projection::ensure_company(&conn, "12345678").unwrap();
    crate::projection::ensure_company(&conn, "87654321").unwrap();
    let id: i64 = conn
        .query_row(
            "SELECT id FROM nos_rede WHERE uuid='politico_42'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(crate::projection::refresh(&conn).unwrap(), count);
    assert_eq!(
        conn.query_row(
            "SELECT id FROM nos_rede WHERE uuid='politico_42'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        id
    );
    assert_eq!(
        conn.query_row(
            "SELECT valor_centavos FROM conexoes_rede WHERE tipo_relacao='DOACAO'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        12345
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM nos_rede WHERE documento='***111222**'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
}
#[test]
fn motor_exige_evidencia_e_expurga_alertas_obsoletos() {
    let p = db();
    let conn = p.get().unwrap();
    politician(&conn);
    conn.execute("INSERT INTO cargos_autoridades(politico_id,orgao,cargo,esfera,data_posse,origem_dado,evidencia_verificada) VALUES (42,'PREFEITURA','PREFEITO','MUNICIPAL','2025-01-01','ATO_OFICIAL',1)",[]).unwrap();
    conn.execute("INSERT INTO receitas_campanha(candidatura_id,doador_cpf_cnpj,doador_nome,valor,data_receita) VALUES (42,'98733344400','DOADOR',1000,'2024-09-01')",[]).unwrap();
    conn.execute("INSERT INTO empresas_qsa(cnpj_basico,cnpj_ordem,cnpj_dv,razao_social,socio_cpf_cnpj_mascarado,socio_nome) VALUES ('12345678','0001','90','EMPRESA','98733344400','DOADOR')",[]).unwrap();
    conn.execute("INSERT INTO contratos_publicos(orgao_contratante,fornecedor_cnpj,valor_contratado,data_assinatura,modalidade) VALUES ('PREFEITURA','12345678000195',10000,'2025-02-01','INEXIGIBILIDADE')",[]).unwrap();
    for (i, time, lat, lon, city) in [
        (1, "2025-02-01T12:00:00Z", -23.55, -46.63, "São Paulo"),
        (2, "2025-02-01T13:00:00Z", -3.73, -38.52, "Fortaleza"),
    ] {
        conn.execute("INSERT INTO despesas_parlamentares(id,casa_legislativa,parlamentar_nome,parlamentar_id,data_emissao,categoria_despesa,fornecedor_nome,fornecedor_cnpj_cpf,valor_liquido,numero_documento,emitido_em,municipio_emissao,uf_emissao,latitude_emissao,longitude_emissao,presencial) VALUES (?1,'CAMARA','MARIA',42,'2025-02-01','ALIMENTACAO','EMPRESA','12345678000195',50,?1,?2,?3,'BR',?4,?5,1)",params![i,time,city,lat,lon]).unwrap();
    }
    for _ in 0..2 {
        crate::alertas::sincronizar_alertas_sistema(&conn).unwrap();
    }
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM alertas_auditoria WHERE tipo IN ('TRIANGULACAO','UBIQUIDADE')",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    let id: i64 = conn
        .query_row(
            "SELECT id FROM alertas_auditoria WHERE tipo='TRIANGULACAO'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    crate::alertas::sincronizar_alertas_sistema(&conn).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT id FROM alertas_auditoria WHERE tipo='TRIANGULACAO'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        id
    );
    conn.execute(
        "UPDATE contratos_publicos SET orgao_contratante='OUTRA PREFEITURA'",
        [],
    )
    .unwrap();
    conn.execute("UPDATE despesas_parlamentares SET presencial=0", [])
        .unwrap();
    crate::alertas::sincronizar_alertas_sistema(&conn).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM alertas_auditoria WHERE tipo IN ('TRIANGULACAO','UBIQUIDADE')",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn exportacao_preserva_csv_com_quebras_de_linha_e_json() {
    let p = db();
    let conn = p.get().unwrap();
    conn.execute(
        "INSERT INTO politicos(nome_completo,nome_urna) VALUES (?1,'MARIA')",
        ["Maria; \"Silva\"\nSantos"],
    )
    .unwrap();
    let file = crate::export::table_file(&conn, "politicos", "csv").unwrap();
    let mut csv = csv::ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(file.reopen().unwrap());
    assert_eq!(
        csv.records().next().unwrap().unwrap().get(3),
        Some("Maria; \"Silva\"\nSantos")
    );
    let file = crate::export::table_file(&conn, "politicos", "json").unwrap();
    let value: serde_json::Value = serde_json::from_reader(file.reopen().unwrap()).unwrap();
    assert_eq!(value[0]["nome_completo"], "Maria; \"Silva\"\nSantos");
}

#[tokio::test]
async fn projecao_e_persistencia_concorrentes_nao_perdem_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let pool = DbPool::open_with_size(directory.path().join("concurrent.db"), 3).unwrap();
    {
        let mut conn = pool.get().unwrap();
        storage::run_migrations(&mut conn).unwrap();
        politician(&conn);
        let tx = storage::transaction_immediate(&mut conn).unwrap();
        for i in 0..2000 {
            tx.execute("INSERT INTO receitas_campanha(candidatura_id,doador_cpf_cnpj,doador_nome,valor) VALUES (42,'98733344400','DOADOR',?1)",[i]).unwrap();
        }
        tx.commit().unwrap();
    }
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let index_pool = pool.clone();
    let index_barrier = barrier.clone();
    let graph = tokio::task::spawn_blocking(move || {
        let conn = index_pool.get().unwrap();
        index_barrier.wait();
        for _ in 0..3 {
            assert_eq!(crate::projection::refresh(&conn).unwrap(), 2000);
        }
    });
    let writer = tokio::task::spawn_blocking(move || {
        let conn = pool.get().unwrap();
        barrier.wait();
        for i in 0..15 {
            conn.execute("INSERT INTO configuracoes_sistema(chave,valor_texto) VALUES ('progress',?1) ON CONFLICT(chave) DO UPDATE SET valor_texto=excluded.valor_texto",[i.to_string()]).unwrap();
        }
    });
    graph.await.unwrap();
    writer.await.unwrap();
}

#[test]
fn cache_projecao_reutiliza_e_invalida_alteracoes_transacionais() {
    let pool = db();
    let conn = pool.get().unwrap();
    politician(&conn);
    crate::projection::refresh_if_needed(&conn).unwrap();
    conn.execute_batch("CREATE TRIGGER forbid_projection BEFORE DELETE ON conexoes_rede WHEN OLD.tipo_relacao='TESTE' BEGIN SELECT RAISE(ABORT,'reconstrucao indevida'); END;").unwrap();
    // Uma relação sentinela torna uma reconstrução indevida observável.
    conn.execute("INSERT INTO conexoes_rede(origem_id,destino_id,tipo_relacao,ano,fonte_dado,metadata_json) SELECT id,id,'TESTE',0,'TESTE',json_object('projection','radar') FROM nos_rede LIMIT 1", []).unwrap();
    crate::projection::refresh_if_needed(&conn).unwrap();
    conn.execute("UPDATE politicos SET foto_url='foto'", [])
        .unwrap();
    crate::projection::refresh_if_needed(&conn).unwrap();
    conn.execute_batch("BEGIN; UPDATE politicos SET nome_completo='OUTRO'; ROLLBACK;")
        .unwrap();
    crate::projection::refresh_if_needed(&conn).unwrap();
    conn.execute("UPDATE politicos SET nome_completo='ALTERADO'", [])
        .unwrap();
    crate::projection::refresh_if_needed(&conn).unwrap();
    let dirty: bool = conn
        .query_row("SELECT dirty FROM projection_state", [], |r| r.get(0))
        .unwrap();
    assert!(!dirty); // A alteração pontual não remove a relação sentinela.
    conn.execute_batch("DROP TRIGGER forbid_projection")
        .unwrap();
    crate::projection::refresh_if_needed(&conn).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT nome FROM nos_rede WHERE uuid='politico_42'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "ALTERADO"
    );
}

#[test]
fn documentos_invalidos_nao_geram_relacoes_financeiras_confirmadas() {
    let pool = db();
    let conn = pool.get().unwrap();
    politician(&conn);
    conn.execute("INSERT INTO receitas_campanha(candidatura_id,doador_cpf_cnpj,doador_nome,valor) VALUES (42,'00000000000','SEM CPF',100),(42,'98733344499','CPF INVALIDO',100)", []).unwrap();
    assert_eq!(crate::projection::refresh(&conn).unwrap(), 0);
}
