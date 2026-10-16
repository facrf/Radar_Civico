use super::*;
use storage::rusqlite::params;
fn fixture() -> storage::DbPool {
    let pool = storage::DbPool::open_in_memory().unwrap();
    let mut conn = pool.get().unwrap();
    storage::run_migrations(&mut conn).unwrap();
    conn.execute_batch("INSERT INTO politicos(id,nome_completo,nome_urna) VALUES(42,'MARIA','MARIA'); INSERT INTO candidaturas(id,politico_id,ano_eleicao,cargo,sigla_partido,uf) VALUES(42,42,2024,'PREFEITO','ABC','SP'); INSERT INTO receitas_campanha(id,candidatura_id,doador_cpf_cnpj,doador_nome,valor) VALUES(1,42,'98733344400','DOADOR A',10),(2,42,'12311122207','DOADOR B',20);").unwrap();
    assert_eq!(refresh(&conn).unwrap(), 2);
    drop(conn);
    pool
}
fn snapshot(conn: &storage::rusqlite::Connection) -> Vec<(String, String, String, i64)> {
    conn.prepare("SELECT s.uuid,t.uuid,e.tipo_relacao,e.valor_centavos FROM conexoes_rede e JOIN nos_rede s ON s.id=e.origem_id JOIN nos_rede t ON t.id=e.destino_id ORDER BY 1,2,3,4").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap().collect::<std::result::Result<_,_>>().unwrap()
}
#[test]
fn incremental_preserva_arestas_nao_alteradas_e_equivale_a_reconstrucao() {
    let pool = fixture();
    let conn = pool.get().unwrap();
    let untouched: i64 = conn
        .query_row(
            "SELECT id FROM conexoes_rede WHERE json_extract(metadata_json,'$.receita_id')=2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute("UPDATE receitas_campanha SET valor=30 WHERE id=1", [])
        .unwrap();
    assert_eq!(refresh_if_needed(&conn).unwrap(), 2);
    assert_eq!(
        conn.query_row(
            "SELECT id FROM conexoes_rede WHERE json_extract(metadata_json,'$.receita_id')=2",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        untouched
    );
    conn.execute("DELETE FROM receitas_campanha WHERE id=1", [])
        .unwrap();
    assert_eq!(refresh_if_needed(&conn).unwrap(), 1);
    conn.execute("INSERT INTO empresas_qsa(cnpj_basico,cnpj_ordem,cnpj_dv,razao_social,socio_nome,socio_cpf_cnpj_mascarado) VALUES('11222333','0001','81','EMPRESA','SOCIO','98733344400')",[]).unwrap();
    conn.execute(
        "UPDATE receitas_campanha SET doador_cpf_cnpj='11222333000181' WHERE id=2",
        [],
    )
    .unwrap();
    assert_eq!(refresh_if_needed(&conn).unwrap(), 2);
    let incremental = snapshot(&conn);
    refresh(&conn).unwrap();
    assert_eq!(snapshot(&conn), incremental);
    conn.execute("DELETE FROM empresas_qsa", []).unwrap();
    assert_eq!(refresh_if_needed(&conn).unwrap(), 1);
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM projection_changes", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn falha_incremental_preserva_projecao_anterior_e_fila_para_nova_tentativa() {
    let pool = fixture();
    let conn = pool.get().unwrap();
    let previous = snapshot(&conn);
    conn.execute("UPDATE receitas_campanha SET valor=99 WHERE id=1", [])
        .unwrap();
    conn.execute_batch("CREATE TRIGGER forbid_edge BEFORE INSERT ON conexoes_rede BEGIN SELECT RAISE(ABORT,'falha controlada'); END").unwrap();
    assert!(refresh_if_needed(&conn).is_err());
    assert_eq!(snapshot(&conn), previous);
    assert_eq!(
        conn.query_row("SELECT dirty FROM projection_state", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    conn.execute_batch("DROP TRIGGER forbid_edge").unwrap();
    assert_eq!(refresh_if_needed(&conn).unwrap(), 2);
    assert_eq!(conn.query_row("SELECT valor_centavos FROM conexoes_rede WHERE json_extract(metadata_json,'$.receita_id')=?1",params![1],|r|r.get::<_,i64>(0)).unwrap(),9900);
}
