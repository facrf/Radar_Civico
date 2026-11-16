use crate::alertas::sincronizar_alertas_sistema_com_parametros;
use storage::{
    rusqlite::{params, Connection},
    DbPool,
};

fn database() -> DbPool {
    let pool = DbPool::open_in_memory().unwrap();
    storage::run_migrations(&mut pool.get().unwrap()).unwrap();
    pool
}
fn auditar(conn: &Connection) -> storage::Result<usize> {
    let regras = crate::config::ParametrosAuditoria {
        concentracao_fornecedor_percentual: 0.0,
        ..Default::default()
    };
    sincronizar_alertas_sistema_com_parametros(conn, &regras)
}
fn despesa(conn: &Connection, nome: &str, doc: &str, valor: f64, data: &str, numero: &str) {
    conn.execute("INSERT INTO despesas_parlamentares(casa_legislativa,parlamentar_nome,data_emissao,categoria_despesa,fornecedor_nome,fornecedor_cnpj_cpf,valor_liquido,numero_documento) VALUES('CAMARA',?1,?2,'SERVICOS','FORNECEDOR TESTE',?3,?4,?5)",params![nome,data,doc,valor,numero]).unwrap();
}

#[test]
fn doacao_circular_agrega_antes_de_cruzar_e_remove_resultado_obsoleto() {
    let pool = database();
    let conn = pool.get().unwrap();
    conn.execute_batch("INSERT INTO politicos(id,nome_completo,nome_urna) VALUES(1,'PARLAMENTAR TESTE','PARLAMENTAR TESTE'); INSERT INTO candidaturas(id,politico_id,ano_eleicao,cargo,sigla_partido,uf) VALUES(1,1,2024,'DEPUTADO','ABC','SP');").unwrap();
    for dia in 1..=2 {
        despesa(
            &conn,
            "PARLAMENTAR TESTE",
            "52998224725",
            3000.0,
            &format!("2024-01-{dia:02}"),
            &format!("NF{dia}"),
        );
    }
    for dia in 1..=3 {
        conn.execute("INSERT INTO receitas_campanha(candidatura_id,doador_cpf_cnpj,doador_nome,valor,data_receita) VALUES(1,'52998224725','FORNECEDOR TESTE',400,?1)",[format!("2024-09-{dia:02}")]).unwrap();
    }
    auditar(&conn).unwrap();
    let quantidade = || {
        conn.query_row(
            "SELECT COUNT(*) FROM alertas_auditoria WHERE tipo='DOACAO_CIRCULAR'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
    };
    assert_eq!(quantidade(), 0, "R$ 6.000 não alcança o piso de R$ 10.000");
    despesa(
        &conn,
        "PARLAMENTAR TESTE",
        "52998224725",
        5000.0,
        "2024-02-01",
        "NF3",
    );
    auditar(&conn).unwrap();
    assert_eq!(quantidade(), 1);
    let (ceap,doacao):(f64,f64)=conn.query_row("SELECT json_extract(detalhes_json,'$.valor_ceap_total'),json_extract(detalhes_json,'$.valor_doacao_total') FROM alertas_auditoria WHERE tipo='DOACAO_CIRCULAR'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!((ceap, doacao), (11000.0, 1200.0));
    auditar(&conn).unwrap();
    assert_eq!(quantidade(), 1);
    conn.execute("UPDATE receitas_campanha SET valor=100", [])
        .unwrap();
    auditar(&conn).unwrap();
    assert_eq!(quantidade(), 0);
}

#[test]
fn falha_ao_salvar_alerta_reverte_auditoria_e_mantem_avaliacao_pendente() {
    let pool = database();
    let conn = pool.get().unwrap();
    for dia in 1..=5 {
        despesa(
            &conn,
            "PARLAMENTAR TESTE",
            "52998224725",
            3000.0,
            &format!("2024-01-{dia:02}"),
            &format!("NF{dia}"),
        );
    }
    conn.execute_batch("CREATE TRIGGER impedir_alerta BEFORE INSERT ON alertas_auditoria WHEN NEW.tipo='FRACIONAMENTO_DESPESAS' BEGIN SELECT RAISE(ABORT,'falha simulada'); END;").unwrap();
    assert!(auditar(&conn).is_err());
    let (pendente, revisao): (i64, i64) = conn
        .query_row("SELECT pendente,revisao FROM auditoria_estado", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((pendente, revisao), (1, 0));
    conn.execute_batch("DROP TRIGGER impedir_alerta").unwrap();
    auditar(&conn).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM alertas_auditoria WHERE tipo='FRACIONAMENTO_DESPESAS'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn fracionamento_inclui_despesas_apos_as_primeiras_dez_mil() {
    let pool = database();
    let mut conn = pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    for i in 0..10001 {
        despesa(
            &tx,
            "PARLAMENTAR TESTE",
            &format!("{i:011}"),
            0.01,
            "2000-01-01",
            &format!("NF{i}"),
        );
    }
    for dia in 1..=3 {
        despesa(
            &tx,
            "PARLAMENTAR TESTE",
            "52998224725",
            4000.0,
            &format!("2024-01-{dia:02}"),
            &format!("RECENTE{dia}"),
        );
    }
    tx.commit().unwrap();
    auditar(&conn).unwrap();
    assert_eq!(conn.query_row("SELECT json_extract(detalhes_json,'$.valor_total') FROM alertas_auditoria WHERE tipo='FRACIONAMENTO_DESPESAS'",[],|r|r.get::<_,f64>(0)).unwrap(),12000.0);
}

#[test]
fn parentesco_inclui_politicos_apos_o_limite_antigo() {
    let pool = database();
    let mut conn = pool.get().unwrap();
    let tx = conn.transaction().unwrap();
    for i in 1..=151 {
        let nome = if i == 151 {
            "ALBERTO GOLDSZTEJN".into()
        } else {
            format!("PESSOA SILVA {i}")
        };
        tx.execute(
            "INSERT INTO politicos(id,nome_completo,nome_urna) VALUES(?1,?2,?2)",
            params![i, nome],
        )
        .unwrap();
        tx.execute("INSERT INTO candidaturas(politico_id,ano_eleicao,cargo,sigla_partido,uf) VALUES(?1,2024,'DEPUTADO','ABC','SP')",[i]).unwrap();
        despesa(
            &tx,
            &nome,
            "11222333000181",
            100.0,
            "2024-01-01",
            &format!("NF{i}"),
        );
    }
    tx.execute("INSERT INTO empresas_qsa(cnpj_basico,cnpj_ordem,cnpj_dv,razao_social,socio_nome,socio_cpf_cnpj_mascarado) VALUES('11222333','0001','81','EMPRESA','BEATRIZ GOLDSZTEJN','***123456**')",[]).unwrap();
    tx.commit().unwrap();
    auditar(&conn).unwrap();
    assert_eq!(conn.query_row("SELECT COUNT(*) FROM alertas_auditoria WHERE tipo='POSSIVEL_PARENTESCO' AND json_extract(detalhes_json,'$.politico_id')=151",[],|r|r.get::<_,i64>(0)).unwrap(),1);
}

#[test]
fn data_invalida_nao_vira_primeiro_de_janeiro_de_1970() {
    let pool = database();
    let conn = pool.get().unwrap();
    despesa(
        &conn,
        "PARLAMENTAR TESTE",
        "52998224725",
        5000.0,
        "data desconhecida",
        "NF1",
    );
    assert!(auditar(&conn).is_err());
    assert_eq!(
        conn.query_row("SELECT pendente FROM auditoria_estado", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
