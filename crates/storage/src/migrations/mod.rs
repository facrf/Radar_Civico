use rusqlite::Connection;
use crate::error::Result;

pub struct Migration {
    pub version: i32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "create_politicos_candidaturas_bens",
        sql: "
            CREATE TABLE IF NOT EXISTS politicos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sq_candidato TEXT UNIQUE,
                cpf_mascarado TEXT,
                nome_completo TEXT NOT NULL,
                nome_urna TEXT NOT NULL,
                data_nascimento TEXT,
                grau_instrucao TEXT,
                ocupacao TEXT,
                foto_blob BLOB,
                foto_mime TEXT DEFAULT 'image/jpeg',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS candidaturas (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                politico_id INTEGER REFERENCES politicos(id),
                ano_eleicao INTEGER NOT NULL,
                cargo TEXT NOT NULL,
                numero_urna INTEGER,
                sigla_partido TEXT NOT NULL,
                uf TEXT NOT NULL,
                municipio TEXT,
                situacao_totalizacao TEXT,
                total_bens_declarados REAL DEFAULT 0.0,
                UNIQUE(politico_id, ano_eleicao, cargo)
            );

            CREATE TABLE IF NOT EXISTS bens_candidato (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                candidatura_id INTEGER REFERENCES candidaturas(id),
                tipo_bem TEXT,
                descricao TEXT,
                valor_declarado REAL NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_candidaturas_politico ON candidaturas(politico_id);
            CREATE INDEX IF NOT EXISTS idx_candidaturas_eleicao ON candidaturas(ano_eleicao, cargo, uf);
            CREATE INDEX IF NOT EXISTS idx_bens_candidatura ON bens_candidato(candidatura_id);
        ",
    },
    Migration {
        version: 2,
        name: "create_receitas_despesas_qsa",
        sql: "
            CREATE TABLE IF NOT EXISTS receitas_campanha (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                candidatura_id INTEGER REFERENCES candidaturas(id),
                doador_cpf_cnpj TEXT NOT NULL,
                doador_nome TEXT NOT NULL,
                valor REAL NOT NULL,
                data_receita TEXT,
                tipo_origem TEXT,
                descricao TEXT
            );

            CREATE TABLE IF NOT EXISTS despesas_campanha (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                candidatura_id INTEGER REFERENCES candidaturas(id),
                fornecedor_cpf_cnpj TEXT NOT NULL,
                fornecedor_nome TEXT NOT NULL,
                valor REAL NOT NULL,
                data_despesa TEXT,
                tipo_despesa TEXT,
                descricao TEXT
            );

            CREATE TABLE IF NOT EXISTS empresas_qsa (
                cnpj_basico TEXT NOT NULL,
                cnpj_ordem TEXT NOT NULL,
                cnpj_dv TEXT NOT NULL,
                razao_social TEXT NOT NULL,
                socio_cpf_cnpj_mascarado TEXT NOT NULL,
                socio_nome TEXT NOT NULL,
                qualificacao_socio TEXT,
                PRIMARY KEY (cnpj_basico, socio_cpf_cnpj_mascarado)
            );

            CREATE INDEX IF NOT EXISTS idx_receitas_candidatura ON receitas_campanha(candidatura_id);
            CREATE INDEX IF NOT EXISTS idx_receitas_doador ON receitas_campanha(doador_cpf_cnpj);
            CREATE INDEX IF NOT EXISTS idx_despesas_candidatura ON despesas_campanha(candidatura_id);
            CREATE INDEX IF NOT EXISTS idx_despesas_fornecedor ON despesas_campanha(fornecedor_cpf_cnpj);
            CREATE INDEX IF NOT EXISTS idx_qsa_socio ON empresas_qsa(socio_cpf_cnpj_mascarado);
        ",
    },
    Migration {
        version: 3,
        name: "create_despesas_parlamentares_contratos_publicos",
        sql: "
            CREATE TABLE IF NOT EXISTS despesas_parlamentares (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                casa_legislativa TEXT NOT NULL,
                parlamentar_nome TEXT NOT NULL,
                parlamentar_cpf_mascarado TEXT,
                data_emissao TEXT NOT NULL,
                categoria_despesa TEXT NOT NULL,
                fornecedor_nome TEXT NOT NULL,
                fornecedor_cnpj_cpf TEXT NOT NULL,
                valor_liquido REAL NOT NULL,
                numero_documento TEXT,
                url_nota_fiscal TEXT,
                detalhes_litros REAL,
                flag_anomalia BOOLEAN DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS contratos_publicos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                orgao_contratante TEXT NOT NULL,
                fornecedor_cnpj TEXT NOT NULL,
                valor_contratado REAL NOT NULL,
                objeto TEXT,
                data_assinatura TEXT,
                data_termino TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_desp_parl_fornecedor ON despesas_parlamentares(fornecedor_cnpj_cpf);
            CREATE INDEX IF NOT EXISTS idx_desp_parl_parlamentar ON despesas_parlamentares(parlamentar_nome);
            CREATE INDEX IF NOT EXISTS idx_contratos_fornecedor ON contratos_publicos(fornecedor_cnpj);
        ",
    },
    Migration {
        version: 4,
        name: "create_nos_rede_conexoes_rede",
        sql: "
            CREATE TABLE IF NOT EXISTS nos_rede (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uuid TEXT UNIQUE NOT NULL,
                tipo TEXT NOT NULL,
                documento TEXT,
                nome TEXT NOT NULL,
                esfera TEXT,
                uf TEXT,
                municipio TEXT,
                metadata_json TEXT
            );

            CREATE TABLE IF NOT EXISTS conexoes_rede (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                origem_id INTEGER REFERENCES nos_rede(id),
                destino_id INTEGER REFERENCES nos_rede(id),
                tipo_relacao TEXT NOT NULL,
                valor REAL DEFAULT 0.0,
                ano INTEGER NOT NULL,
                data_evento TEXT,
                fonte_dado TEXT NOT NULL,
                metadata_json TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_conexoes_origem ON conexoes_rede(origem_id);
            CREATE INDEX IF NOT EXISTS idx_conexoes_destino ON conexoes_rede(destino_id);
            CREATE INDEX IF NOT EXISTS idx_conexoes_busca ON conexoes_rede(tipo_relacao, ano);
            CREATE INDEX IF NOT EXISTS idx_nos_documento ON nos_rede(documento);
            CREATE INDEX IF NOT EXISTS idx_nos_tipo ON nos_rede(tipo);
        ",
    },
    Migration {
        version: 5,
        name: "create_diario_registros_alertas",
        sql: "
            CREATE TABLE IF NOT EXISTS cache_consultas_diario (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                doador_cpf_cnpj TEXT NOT NULL,
                termo_pesquisado TEXT NOT NULL,
                municipio_uf TEXT NOT NULL,
                ocorrencias_encontradas INTEGER DEFAULT 0,
                payload_json TEXT,
                data_consulta TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_cache_doador ON cache_consultas_diario(doador_cpf_cnpj);

            CREATE TABLE IF NOT EXISTS registros_profissionais (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pessoa_nome TEXT NOT NULL,
                cpf_mascarado TEXT,
                orgao_emissor TEXT NOT NULL,
                numero_registro TEXT NOT NULL,
                seccional_uf TEXT NOT NULL,
                situacao_registro TEXT NOT NULL,
                tipo_inscricao TEXT,
                data_consulta TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                payload_json TEXT,
                UNIQUE(orgao_emissor, seccional_uf, numero_registro)
            );

            CREATE INDEX IF NOT EXISTS idx_reg_prof_nome ON registros_profissionais(pessoa_nome);
            CREATE INDEX IF NOT EXISTS idx_reg_prof_status ON registros_profissionais(orgao_emissor, situacao_registro);

            CREATE TABLE IF NOT EXISTS alertas_incompatibilidade (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                registro_profissional_id INTEGER REFERENCES registros_profissionais(id),
                politico_ou_gestor_id INTEGER REFERENCES nos_rede(id),
                cargo_ocupado TEXT NOT NULL,
                orgao_lotacao TEXT NOT NULL,
                data_nomeacao TEXT NOT NULL,
                motivo_incompatibilidade TEXT NOT NULL,
                status_apuracao TEXT DEFAULT 'PENDENTE'
            );

            CREATE INDEX IF NOT EXISTS idx_alertas_registro ON alertas_incompatibilidade(registro_profissional_id);
            CREATE INDEX IF NOT EXISTS idx_alertas_gestor ON alertas_incompatibilidade(politico_ou_gestor_id);
        ",
    },
    Migration {
        version: 6,
        name: "create_fts5_tables_and_triggers",
        sql: "
            CREATE VIRTUAL TABLE IF NOT EXISTS politicos_fts USING fts5(
                politico_id UNINDEXED,
                nome_completo,
                nome_urna,
                sq_candidato
            );

            CREATE TRIGGER IF NOT EXISTS trg_politicos_ai AFTER INSERT ON politicos BEGIN
                INSERT INTO politicos_fts(politico_id, nome_completo, nome_urna, sq_candidato)
                VALUES (new.id, new.nome_completo, new.nome_urna, new.sq_candidato);
            END;

            CREATE TRIGGER IF NOT EXISTS trg_politicos_ad AFTER DELETE ON politicos BEGIN
                DELETE FROM politicos_fts WHERE politico_id = old.id;
            END;

            CREATE TRIGGER IF NOT EXISTS trg_politicos_au AFTER UPDATE ON politicos BEGIN
                DELETE FROM politicos_fts WHERE politico_id = old.id;
                INSERT INTO politicos_fts(politico_id, nome_completo, nome_urna, sq_candidato)
                VALUES (new.id, new.nome_completo, new.nome_urna, new.sq_candidato);
            END;

            CREATE VIRTUAL TABLE IF NOT EXISTS fornecedores_fts USING fts5(
                fornecedor_cpf_cnpj,
                fornecedor_nome
            );

            CREATE TRIGGER IF NOT EXISTS trg_despesas_campanha_fornecedores_ai AFTER INSERT ON despesas_campanha BEGIN
                INSERT INTO fornecedores_fts(fornecedor_cpf_cnpj, fornecedor_nome)
                VALUES (new.fornecedor_cpf_cnpj, new.fornecedor_nome);
            END;

            CREATE TRIGGER IF NOT EXISTS trg_despesas_parlamentares_fornecedores_ai AFTER INSERT ON despesas_parlamentares BEGIN
                INSERT INTO fornecedores_fts(fornecedor_cpf_cnpj, fornecedor_nome)
                VALUES (new.fornecedor_cnpj_cpf, new.fornecedor_nome);
            END;
        ",
    },
    Migration {
        version: 7,
        name: "create_alertas_auditoria",
        sql: "
            CREATE TABLE IF NOT EXISTS alertas_auditoria (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tipo TEXT NOT NULL,
                severidade TEXT NOT NULL,
                titulo TEXT NOT NULL,
                descricao TEXT NOT NULL,
                alvo_nome TEXT NOT NULL,
                alvo_documento TEXT,
                municipio TEXT,
                uf TEXT,
                ano INTEGER,
                valor_envolvido REAL,
                fonte_dado TEXT NOT NULL,
                detalhes_json TEXT,
                data_criacao TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_alertas_auditoria_filtro ON alertas_auditoria(ano, municipio);
            CREATE INDEX IF NOT EXISTS idx_alertas_auditoria_severidade ON alertas_auditoria(severidade);
            CREATE INDEX IF NOT EXISTS idx_alertas_auditoria_tipo ON alertas_auditoria(tipo);
        ",
    },
    Migration {
        version: 8,
        name: "create_historico_sincronizacao",
        sql: "
            CREATE TABLE IF NOT EXISTS historico_sincronizacao (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                fonte TEXT NOT NULL,
                status TEXT NOT NULL,
                detalhes TEXT,
                registros_afetados INTEGER DEFAULT 0,
                data_inicio TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                data_fim TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_hist_sinc_data ON historico_sincronizacao(data_inicio DESC);
        ",
    },
];

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );",
        [],
    )?;

    for migration in MIGRATIONS {
        let applied: bool = conn
            .query_row(
                "SELECT 1 FROM _migrations WHERE version = ?1",
                [migration.version],
                |_| Ok(()),
            )
            .map(|_| true)
            .unwrap_or(false);

        if !applied {
            let tx = conn.transaction()?;
            tx.execute_batch(migration.sql)?;
            tx.execute(
                "INSERT INTO _migrations (version, name) VALUES (?1, ?2)",
                (migration.version, migration.name),
            )?;
            tx.commit()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::DbPool;

    #[test]
    fn test_migrations_politicos_candidaturas_bens() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;

        run_migrations(&mut conn)?;

        // Ensure tables exist and can store/retrieve data
        let foto_sample = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        conn.execute(
            "INSERT INTO politicos (sq_candidato, cpf_mascarado, nome_completo, nome_urna, foto_blob)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            ("12345", "***.456.789-**", "FULANO DE TAL", "FULANO", &foto_sample),
        )?;

        let politico_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, numero_urna, sigla_partido, uf, municipio)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (politico_id, 2024, "PREFEITO", 99, "PARTIDO", "SP", "SAO PAULO"),
        )?;

        let candidatura_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO bens_candidato (candidatura_id, tipo_bem, descricao, valor_declarado)
             VALUES (?1, ?2, ?3, ?4)",
            (candidatura_id, "VEICULO", "AUTOMOVEL FIAT UNO", 35000.0),
        )?;

        let count: i64 = conn.query_row("SELECT count(*) FROM bens_candidato", [], |r| r.get(0))?;
        assert_eq!(count, 1);

        // Verify idempotency
        run_migrations(&mut conn)?;

        Ok(())
    }

    #[test]
    fn test_migrations_receitas_despesas_qsa() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;

        run_migrations(&mut conn)?;

        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna) VALUES ('CANDIDATO', 'CAND')",
            [],
        )?;
        let pol_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf)
             VALUES (?1, 2024, 'VEREADOR', 'PART', 'SP')",
            [pol_id],
        )?;
        let cand_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO receitas_campanha (candidatura_id, doador_cpf_cnpj, doador_nome, valor)
             VALUES (?1, '11122233344', 'DOADOR SILVA', 5000.0)",
            [cand_id],
        )?;

        conn.execute(
            "INSERT INTO despesas_campanha (candidatura_id, fornecedor_cpf_cnpj, fornecedor_nome, valor)
             VALUES (?1, '12345678000199', 'GRAFICA XYZ', 3000.0)",
            [cand_id],
        )?;

        conn.execute(
            "INSERT INTO empresas_qsa (cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_cpf_cnpj_mascarado, socio_nome)
             VALUES ('12345678', '0001', '99', 'GRAFICA XYZ LTDA', '***.222.333-**', 'SOCIO EMPRESA')",
            [],
        )?;

        let rec_count: i64 = conn.query_row("SELECT count(*) FROM receitas_campanha", [], |r| r.get(0))?;
        let desp_count: i64 = conn.query_row("SELECT count(*) FROM despesas_campanha", [], |r| r.get(0))?;
        let qsa_count: i64 = conn.query_row("SELECT count(*) FROM empresas_qsa", [], |r| r.get(0))?;

        assert_eq!(rec_count, 1);
        assert_eq!(desp_count, 1);
        assert_eq!(qsa_count, 1);

        Ok(())
    }

    #[test]
    fn test_migrations_despesas_parlamentares_contratos_publicos() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;

        run_migrations(&mut conn)?;

        conn.execute(
            "INSERT INTO despesas_parlamentares (
                casa_legislativa, parlamentar_nome, parlamentar_cpf_mascarado, data_emissao,
                categoria_despesa, fornecedor_nome, fornecedor_cnpj_cpf, valor_liquido,
                numero_documento, url_nota_fiscal, detalhes_litros, flag_anomalia
             ) VALUES (
                'CAMARA', 'DEPUTADO TESTE', '***.123.456-**', '2024-05-10',
                'COMBUSTIVEIS E LUBRIFICANTES', 'POSTO CENTRAL', '00123456000100', 450.50,
                'NF-1234', 'http://nfe.gov.br/1234', 85.5, 1
             )",
            [],
        )?;

        conn.execute(
            "INSERT INTO contratos_publicos (
                orgao_contratante, fornecedor_cnpj, valor_contratado, objeto, data_assinatura, data_termino
             ) VALUES (
                'PREFEITURA MUNICIPAL', '00123456000100', 150000.0, 'FORNECIMENTO DE COMBUSTIVEL', '2024-06-01', '2025-06-01'
             )",
            [],
        )?;

        let desp_count: i64 = conn.query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| r.get(0))?;
        let litros: f64 = conn.query_row("SELECT detalhes_litros FROM despesas_parlamentares WHERE id = 1", [], |r| r.get(0))?;
        let cont_count: i64 = conn.query_row("SELECT count(*) FROM contratos_publicos", [], |r| r.get(0))?;

        assert_eq!(desp_count, 1);
        assert_eq!(litros, 85.5);
        assert_eq!(cont_count, 1);

        Ok(())
    }

    #[test]
    fn test_migrations_nos_rede_conexoes_rede() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;

        run_migrations(&mut conn)?;

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, documento, nome, esfera, uf, municipio)
             VALUES ('node-1', 'POLITICO', '***123***', 'POLITICO ALVO', 'MUNICIPAL', 'SP', 'CAMPINAS')",
            [],
        )?;
        let id1 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, documento, nome, esfera, uf, municipio)
             VALUES ('node-2', 'EMPRESA', '11222333000199', 'EMPRESA FORNECEDORA', 'MUNICIPAL', 'SP', 'CAMPINAS')",
            [],
        )?;
        let id2 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO conexoes_rede (origem_id, destino_id, tipo_relacao, valor, ano, fonte_dado)
             VALUES (?1, ?2, 'CONTRATO_PUBLICO', 50000.0, 2024, 'PNCP')",
            (id1, id2),
        )?;

        let nos_count: i64 = conn.query_row("SELECT count(*) FROM nos_rede", [], |r| r.get(0))?;
        let conexoes_count: i64 = conn.query_row("SELECT count(*) FROM conexoes_rede", [], |r| r.get(0))?;

        assert_eq!(nos_count, 2);
        assert_eq!(conexoes_count, 1);

        Ok(())
    }

    #[test]
    fn test_migrations_diario_registros_alertas() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;

        run_migrations(&mut conn)?;

        conn.execute(
            "INSERT INTO cache_consultas_diario (doador_cpf_cnpj, termo_pesquisado, municipio_uf, ocorrencias_encontradas, payload_json)
             VALUES ('12345678901', 'JOAO DA SILVA', 'CAMPINAS-SP', 2, '{\"resumo\": \"nomeacao\"}')",
            [],
        )?;

        conn.execute(
            "INSERT INTO registros_profissionais (pessoa_nome, cpf_mascarado, orgao_emissor, numero_registro, seccional_uf, situacao_registro, tipo_inscricao)
             VALUES ('JOAO DA SILVA', '***.456.789-**', 'OAB', '123456', 'SP', 'REGULAR', 'ADVOGADO')",
            [],
        )?;
        let reg_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO nos_rede (uuid, tipo, nome) VALUES ('gestor-1', 'POLITICO', 'SECRETARIO MUNICIPAL')",
            [],
        )?;
        let gestor_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO alertas_incompatibilidade (registro_profissional_id, politico_ou_gestor_id, cargo_ocupado, orgao_lotacao, data_nomeacao, motivo_incompatibilidade)
             VALUES (?1, ?2, 'SECRETARIO DE GOVERNO', 'PREFEITURA MUNICIPAL', '2024-01-02', 'Violacao Art. 28, III, Lei 8.906/94')",
            (reg_id, gestor_id),
        )?;

        let cache_count: i64 = conn.query_row("SELECT count(*) FROM cache_consultas_diario", [], |r| r.get(0))?;
        let reg_count: i64 = conn.query_row("SELECT count(*) FROM registros_profissionais", [], |r| r.get(0))?;
        let alerta_count: i64 = conn.query_row("SELECT count(*) FROM alertas_incompatibilidade", [], |r| r.get(0))?;

        assert_eq!(cache_count, 1);
        assert_eq!(reg_count, 1);
        assert_eq!(alerta_count, 1);

        Ok(())
    }

    #[test]
    fn test_migrations_fts5_and_triggers() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;

        run_migrations(&mut conn)?;

        // Insert into politicos and verify automatic trigger sync to politicos_fts
        conn.execute(
            "INSERT INTO politicos (sq_candidato, nome_completo, nome_urna)
             VALUES ('SQ999', 'CARLOS EDUARDO SILVA', 'DUDU')",
            [],
        )?;
        let pol_id = conn.last_insert_rowid();

        // Search politicos_fts
        let found_id: i64 = conn.query_row(
            "SELECT politico_id FROM politicos_fts WHERE politicos_fts MATCH 'DUDU*'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(found_id, pol_id);

        // Update politicos and check updated fts
        conn.execute(
            "UPDATE politicos SET nome_urna = 'CORONEL' WHERE id = ?1",
            [pol_id],
        )?;

        let count_old: i64 = conn.query_row(
            "SELECT count(*) FROM politicos_fts WHERE politicos_fts MATCH 'DUDU*'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(count_old, 0);

        let count_new: i64 = conn.query_row(
            "SELECT count(*) FROM politicos_fts WHERE politicos_fts MATCH 'CORONEL*'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(count_new, 1);

        // Insert into despesas_parlamentares and verify trigger sync to fornecedores_fts
        conn.execute(
            "INSERT INTO despesas_parlamentares (
                casa_legislativa, parlamentar_nome, data_emissao, categoria_despesa,
                fornecedor_nome, fornecedor_cnpj_cpf, valor_liquido
             ) VALUES (
                'CAMARA', 'DEPUTADO TESTE', '2024-05-10', 'COMBUSTIVEIS',
                'AUTO POSTO ALVORADA', '12345678000199', 200.0
             )",
            [],
        )?;

        let found_fornecedor: String = conn.query_row(
            "SELECT fornecedor_nome FROM fornecedores_fts WHERE fornecedores_fts MATCH 'ALVORADA*'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(found_fornecedor, "AUTO POSTO ALVORADA");

        Ok(())
    }

    #[test]
    fn test_migrations_historico_sincronizacao() -> Result<()> {
        let mut conn = Connection::open_in_memory()?;
        run_migrations(&mut conn)?;

        conn.execute(
            "INSERT INTO historico_sincronizacao (fonte, status, detalhes, registros_afetados)
             VALUES ('TSE', 'CONCLUIDO', '150 registros ingeridos', 150)",
            [],
        )?;

        let (fonte, status, afetados): (String, String, i64) = conn.query_row(
            "SELECT fonte, status, registros_afetados FROM historico_sincronizacao ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

        assert_eq!(fonte, "TSE");
        assert_eq!(status, "CONCLUIDO");
        assert_eq!(afetados, 150);

        Ok(())
    }
}
