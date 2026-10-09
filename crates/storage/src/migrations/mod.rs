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
    Migration {
        version: 9,
        name: "create_configuracoes_sistema",
        sql: "
            CREATE TABLE IF NOT EXISTS configuracoes_sistema (
                chave TEXT PRIMARY KEY,
                valor_texto TEXT,
                valor_blob BLOB,
                mime_type TEXT,
                atualizado_em TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
        ",
    },
    Migration {
        version: 10,
        name: "create_beneficios_emergenciais_alertas",
        sql: "
            CREATE TABLE IF NOT EXISTS beneficios_emergenciais (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                cpf_mascarado TEXT NOT NULL,
                nome_beneficiario TEXT NOT NULL,
                municipio TEXT,
                uf TEXT,
                mes_disponibilizacao TEXT NOT NULL,
                parcela TEXT,
                valor REAL NOT NULL,
                enquadramento TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS alertas_beneficio_indevido (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                politico_id INTEGER REFERENCES politicos(id),
                beneficio_id INTEGER REFERENCES beneficios_emergenciais(id),
                motivo TEXT NOT NULL,
                detalhes TEXT,
                valor_recebido REAL NOT NULL,
                total_bens REAL,
                cargo_ou_mandato TEXT,
                ano_exercicio INTEGER,
                status_analise TEXT DEFAULT 'PENDENTE',
                data_alerta TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_beneficios_cpf ON beneficios_emergenciais(cpf_mascarado);
            CREATE INDEX IF NOT EXISTS idx_beneficios_nome ON beneficios_emergenciais(nome_beneficiario);
            CREATE INDEX IF NOT EXISTS idx_beneficios_uf_mun ON beneficios_emergenciais(uf, municipio);
            CREATE INDEX IF NOT EXISTS idx_beneficios_mes ON beneficios_emergenciais(mes_disponibilizacao);

            CREATE INDEX IF NOT EXISTS idx_alertas_beneficio_politico ON alertas_beneficio_indevido(politico_id);
            CREATE INDEX IF NOT EXISTS idx_alertas_beneficio_beneficio ON alertas_beneficio_indevido(beneficio_id);
            CREATE INDEX IF NOT EXISTS idx_alertas_beneficio_motivo ON alertas_beneficio_indevido(motivo);
        ",
    },
    Migration {
        version: 11,
        name: "create_municipios_ibge",
        sql: "
            CREATE TABLE IF NOT EXISTS municipios_ibge (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                codigo_ibge INTEGER UNIQUE,
                nome TEXT NOT NULL,
                uf TEXT NOT NULL,
                latitude REAL NOT NULL,
                longitude REAL NOT NULL,
                is_capital BOOLEAN DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_municipios_uf ON municipios_ibge(uf);
            CREATE INDEX IF NOT EXISTS idx_municipios_nome ON municipios_ibge(nome);

            INSERT OR IGNORE INTO municipios_ibge (codigo_ibge, nome, uf, latitude, longitude, is_capital) VALUES
            (5300108, 'Brasília', 'DF', -15.793889, -47.882778, 1),
            (3550308, 'São Paulo', 'SP', -23.550520, -46.633308, 1),
            (3304557, 'Rio de Janeiro', 'RJ', -22.906847, -43.172896, 1),
            (3106200, 'Belo Horizonte', 'MG', -19.920831, -43.937778, 1),
            (4314902, 'Porto Alegre', 'RS', -30.034647, -51.217658, 1),
            (4106902, 'Curitiba', 'PR', -25.428954, -49.267137, 1),
            (4205407, 'Florianópolis', 'SC', -27.595378, -48.548050, 1),
            (2927408, 'Salvador', 'BA', -12.977749, -38.501630, 1),
            (2611606, 'Recife', 'PE', -8.047562, -34.876964, 1),
            (2304400, 'Fortaleza', 'CE', -3.731862, -38.526671, 1),
            (5208707, 'Goiânia', 'GO', -16.686891, -49.264794, 1),
            (5103403, 'Cuiabá', 'MT', -15.601411, -56.097892, 1),
            (5002704, 'Campo Grande', 'MS', -20.469711, -54.620121, 1),
            (1501402, 'Belém', 'PA', -1.455755, -48.490180, 1),
            (2111300, 'São Luís', 'MA', -2.530730, -44.306800, 1),
            (2507507, 'João Pessoa', 'PB', -7.119496, -34.845012, 1),
            (2408102, 'Natal', 'RN', -5.794480, -35.211000, 1),
            (2704302, 'Maceió', 'AL', -9.665800, -35.735300, 1),
            (2211001, 'Teresina', 'PI', -5.091900, -42.803400, 1),
            (2800308, 'Aracaju', 'SE', -10.947200, -37.073100, 1),
            (3205309, 'Vitória', 'ES', -20.315500, -40.312800, 1),
            (1100205, 'Porto Velho', 'RO', -8.761900, -63.903900, 1),
            (1721000, 'Palmas', 'TO', -10.249100, -48.324300, 1),
            (1600303, 'Macapá', 'AP', 0.035500, -51.070500, 1),
            (1400100, 'Boa Vista', 'RR', 2.823500, -60.675800, 1),
            (1200401, 'Rio Branco', 'AC', -9.975300, -67.824900, 1),
            (1302603, 'Manaus', 'AM', -3.119028, -60.021731, 1),
            (4305108, 'Caxias do Sul', 'RS', -29.1678, -51.1794, 0),
            (4314407, 'Pelotas', 'RS', -31.7654, -52.3376, 0),
            (4316907, 'Santa Maria', 'RS', -29.6842, -53.8069, 0),
            (4304606, 'Canoas', 'RS', -29.9178, -51.1836, 0),
            (3509502, 'Campinas', 'SP', -22.9099, -47.0626, 0),
            (3548500, 'Santos', 'SP', -23.9608, -46.3336, 0),
            (3543402, 'Ribeirão Preto', 'SP', -21.1775, -47.8103, 0),
            (3549904, 'São José dos Campos', 'SP', -23.1794, -45.8869, 0),
            (3303302, 'Niterói', 'RJ', -22.8833, -43.1036, 0),
            (3170206, 'Uberlândia', 'MG', -18.9186, -48.2772, 0),
            (3136702, 'Juiz de Fora', 'MG', -21.7642, -43.3497, 0),
            (4113700, 'Londrina', 'PR', -23.3045, -51.1696, 0),
            (4115200, 'Maringá', 'PR', -23.4209, -51.9331, 0),
            (4209102, 'Joinville', 'SC', -26.3045, -48.8487, 0),
            (4202404, 'Blumenau', 'SC', -26.9194, -49.0661, 0),
            (2910800, 'Feira de Santana', 'BA', -12.2667, -38.9667, 0),
            (5201108, 'Anápolis', 'GO', -16.3267, -48.9533, 0);
        ",
    },
    Migration {
        version: 12,
        name: "normalizar_cargos_partidos_e_indices_candidaturas",
        sql: "
            -- Normalizar códigos de cargos do TSE (11 = PREFEITO, 12 = VICE-PREFEITO, 13 = VEREADOR)
            UPDATE candidaturas SET cargo = 'VEREADOR' WHERE cargo = '13';
            UPDATE candidaturas SET cargo = 'PREFEITO' WHERE cargo = '11';
            UPDATE candidaturas SET cargo = 'VICE-PREFEITO' WHERE cargo = '12';

            -- Normalizar números eleitorais de partidos para siglas oficiais
            UPDATE candidaturas SET sigla_partido = 'REPUBLICANOS' WHERE sigla_partido = '10';
            UPDATE candidaturas SET sigla_partido = 'PP' WHERE sigla_partido = '11';
            UPDATE candidaturas SET sigla_partido = 'PDT' WHERE sigla_partido = '12';
            UPDATE candidaturas SET sigla_partido = 'PT' WHERE sigla_partido = '13';
            UPDATE candidaturas SET sigla_partido = 'PTB' WHERE sigla_partido = '14';
            UPDATE candidaturas SET sigla_partido = 'MDB' WHERE sigla_partido = '15';
            UPDATE candidaturas SET sigla_partido = 'PSTU' WHERE sigla_partido = '16';
            UPDATE candidaturas SET sigla_partido = 'REDE' WHERE sigla_partido = '18';
            UPDATE candidaturas SET sigla_partido = 'PODE' WHERE sigla_partido = '20';
            UPDATE candidaturas SET sigla_partido = 'PCB' WHERE sigla_partido = '21';
            UPDATE candidaturas SET sigla_partido = 'PL' WHERE sigla_partido = '22';
            UPDATE candidaturas SET sigla_partido = 'CIDADANIA' WHERE sigla_partido = '23';
            UPDATE candidaturas SET sigla_partido = 'PRD' WHERE sigla_partido = '25';
            UPDATE candidaturas SET sigla_partido = 'DC' WHERE sigla_partido = '27';
            UPDATE candidaturas SET sigla_partido = 'PRTB' WHERE sigla_partido = '28';
            UPDATE candidaturas SET sigla_partido = 'PCO' WHERE sigla_partido = '29';
            UPDATE candidaturas SET sigla_partido = 'NOVO' WHERE sigla_partido = '30';
            UPDATE candidaturas SET sigla_partido = 'MOBILIZA' WHERE sigla_partido = '33';
            UPDATE candidaturas SET sigla_partido = 'PMB' WHERE sigla_partido = '35';
            UPDATE candidaturas SET sigla_partido = 'AGIR' WHERE sigla_partido = '36';
            UPDATE candidaturas SET sigla_partido = 'PSB' WHERE sigla_partido = '40';
            UPDATE candidaturas SET sigla_partido = 'PV' WHERE sigla_partido = '43';
            UPDATE candidaturas SET sigla_partido = 'UNIÃO' WHERE sigla_partido = '44';
            UPDATE candidaturas SET sigla_partido = 'PSDB' WHERE sigla_partido = '45';
            UPDATE candidaturas SET sigla_partido = 'PSOL' WHERE sigla_partido = '50';
            UPDATE candidaturas SET sigla_partido = 'PSD' WHERE sigla_partido = '55';
            UPDATE candidaturas SET sigla_partido = 'PCdoB' WHERE sigla_partido = '65';
            UPDATE candidaturas SET sigla_partido = 'AVANTE' WHERE sigla_partido = '70';
            UPDATE candidaturas SET sigla_partido = 'SOLIDARIEDADE' WHERE sigla_partido = '77';
            UPDATE candidaturas SET sigla_partido = 'UP' WHERE sigla_partido = '80';

            -- Índices essenciais para buscas por cargo, partido, UF e sequencial
            CREATE INDEX IF NOT EXISTS idx_candidaturas_cargo ON candidaturas(cargo);
            CREATE INDEX IF NOT EXISTS idx_candidaturas_politico_id ON candidaturas(politico_id);
            CREATE INDEX IF NOT EXISTS idx_candidaturas_sigla_partido ON candidaturas(sigla_partido);
            CREATE INDEX IF NOT EXISTS idx_candidaturas_uf ON candidaturas(uf);
            CREATE INDEX IF NOT EXISTS idx_politicos_sq ON politicos(sq_candidato);
            CREATE INDEX IF NOT EXISTS idx_politicos_nome_urna ON politicos(nome_urna);
            CREATE INDEX IF NOT EXISTS idx_politicos_nome_completo ON politicos(nome_completo);
        ",
    },
    Migration {
        version: 13,
        name: "adiciona_foto_url_politicos",
        sql: "
            ALTER TABLE politicos ADD COLUMN foto_url TEXT;
            CREATE INDEX IF NOT EXISTS idx_politicos_foto_url ON politicos(foto_url);
        ",
    },
    Migration {
        version: 14,
        name: "deduplicar_e_criar_indice_unico_despesas_parlamentares",
        sql: "
            -- Remove duplicatas históricas preservando o registro de menor id
            DELETE FROM despesas_parlamentares
            WHERE id NOT IN (
                SELECT MIN(id)
                FROM despesas_parlamentares
                GROUP BY casa_legislativa, parlamentar_nome, data_emissao, fornecedor_cnpj_cpf, valor_liquido, COALESCE(numero_documento, '')
            );

            -- Cria índice único idempotente para impedir novas duplicatas
            CREATE UNIQUE INDEX IF NOT EXISTS idx_desp_parl_dedup ON despesas_parlamentares(
                casa_legislativa, parlamentar_nome, data_emissao, fornecedor_cnpj_cpf, valor_liquido, COALESCE(numero_documento, '')
            );
        ",
    },
    Migration {
        version: 15,
        name: "criar_indices_busca_empresas_qsa",
        sql: "
            CREATE INDEX IF NOT EXISTS idx_qsa_razao ON empresas_qsa(razao_social);
            CREATE INDEX IF NOT EXISTS idx_qsa_socio_nome ON empresas_qsa(socio_nome);
        ",
    },
    Migration {
        version: 16,
        name: "criar_indice_cargo_e_normalizar_candidaturas",
        sql: "
            CREATE INDEX IF NOT EXISTS idx_candidaturas_cargo ON candidaturas(cargo);

            UPDATE candidaturas SET cargo = 'PRESIDENTE' WHERE cargo = '1';
            UPDATE candidaturas SET cargo = 'VICE-PRESIDENTE' WHERE cargo = '2';
            UPDATE candidaturas SET cargo = 'GOVERNADOR' WHERE cargo = '3';
            UPDATE candidaturas SET cargo = 'VICE-GOVERNADOR' WHERE cargo = '4';
            UPDATE candidaturas SET cargo = 'SENADOR' WHERE cargo = '5';
            UPDATE candidaturas SET cargo = 'DEPUTADO FEDERAL' WHERE cargo = '6';
            UPDATE candidaturas SET cargo = 'DEPUTADO ESTADUAL' WHERE cargo = '7';
            UPDATE candidaturas SET cargo = 'DEPUTADO DISTRITAL' WHERE cargo = '8';
        ",
    },
    Migration {
        version: 17,
        name: "criar_indices_parciais_e_enriquecimento_qsa",
        sql: "
            -- Índice parcial para busca rápida de CPF real (ignora NULL e '-4' da LGPD)
            CREATE INDEX IF NOT EXISTS idx_politicos_cpf_valido ON politicos(cpf_mascarado)
            WHERE cpf_mascarado IS NOT NULL AND cpf_mascarado != '-4' AND cpf_mascarado != '';

            -- Índices essenciais para consultas analíticas de CEAP e fornecedores
            CREATE INDEX IF NOT EXISTS idx_despesas_parlamentar_nome ON despesas_parlamentares(parlamentar_nome);
            CREATE INDEX IF NOT EXISTS idx_despesas_parlamentar_cnpj ON despesas_parlamentares(fornecedor_cnpj_cpf);
            CREATE INDEX IF NOT EXISTS idx_despesas_campanha_fornecedor ON despesas_campanha(fornecedor_cpf_cnpj);
            CREATE INDEX IF NOT EXISTS idx_receitas_campanha_doador ON receitas_campanha(doador_cpf_cnpj);
            CREATE INDEX IF NOT EXISTS idx_contratos_valor ON contratos_publicos(valor_contratado);

            -- Enriquecimento opcional de empresas QSA para data de abertura e capital social
            ALTER TABLE empresas_qsa ADD COLUMN data_inicio_atividade TEXT;
            ALTER TABLE empresas_qsa ADD COLUMN capital_social REAL DEFAULT 0.0;
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
            let tx = crate::connection::transaction_immediate(conn)?;
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

    #[test]
    fn test_migrations_configuracoes_sistema() -> Result<()> {
        let mut conn = Connection::open_in_memory()?;
        run_migrations(&mut conn)?;

        let mock_icon_blob = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        conn.execute(
            "INSERT INTO configuracoes_sistema (chave, valor_texto, valor_blob, mime_type)
             VALUES ('app_icon', 'icone_custom.png', ?1, 'image/png')",
            [&mock_icon_blob],
        )?;

        let (chave, mime, blob): (String, String, Vec<u8>) = conn.query_row(
            "SELECT chave, mime_type, valor_blob FROM configuracoes_sistema WHERE chave = 'app_icon'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

        assert_eq!(chave, "app_icon");
        assert_eq!(mime, "image/png");
        assert_eq!(blob, mock_icon_blob);

        Ok(())
    }

    #[test]
    fn test_migrations_beneficios_emergenciais() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;
        run_migrations(&mut conn)?;

        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna, cpf_mascarado)
             VALUES ('DEPUTADO FRAUDADOR', 'FRAUDADOR', '***.111.222-**')",
            [],
        )?;
        let pol_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO beneficios_emergenciais (cpf_mascarado, nome_beneficiario, municipio, uf, mes_disponibilizacao, parcela, valor, enquadramento)
             VALUES ('***.111.222-**', 'DEPUTADO FRAUDADOR', 'BRASILIA', 'DF', '202005', '1ª PARCELA', 600.0, 'EXTRA CAD')",
            [],
        )?;
        let ben_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO alertas_beneficio_indevido (politico_id, beneficio_id, motivo, detalhes, valor_recebido, total_bens, cargo_ou_mandato, ano_exercicio)
             VALUES (?1, ?2, 'MANDATO_VIGENTE', 'Recebeu Auxilio Emergencial ocupando cargo eletivo', 600.0, 500000.0, 'DEPUTADO FEDERAL', 2020)",
            (pol_id, ben_id),
        )?;

        let ben_count: i64 = conn.query_row("SELECT count(*) FROM beneficios_emergenciais", [], |r| r.get(0))?;
        let alerta_count: i64 = conn.query_row("SELECT count(*) FROM alertas_beneficio_indevido", [], |r| r.get(0))?;

        assert_eq!(ben_count, 1);
        assert_eq!(alerta_count, 1);

        // Verify idempotency of migration 10
        run_migrations(&mut conn)?;

        Ok(())
    }

    #[test]
    fn test_migrations_municipios_ibge() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;
        run_migrations(&mut conn)?;

        let count: i64 = conn.query_row("SELECT count(*) FROM municipios_ibge", [], |r| r.get(0))?;
        assert!(count >= 27, "Deveria ter ao menos as 27 capitais inseridas");

        let (lat, lon): (f64, f64) = conn.query_row(
            "SELECT latitude, longitude FROM municipios_ibge WHERE uf = 'DF' AND nome = 'Brasília'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        assert!((lat - (-15.793889)).abs() < 0.001);
        assert!((lon - (-47.882778)).abs() < 0.001);

        // Verify idempotency
        run_migrations(&mut conn)?;

        Ok(())
    }

    #[test]
    fn test_migrations_normalizar_candidaturas() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;
        run_migrations(&mut conn)?;

        // Insere registros com codigos numéricos para testar a normalização
        conn.execute(
            "INSERT INTO politicos (nome_completo, nome_urna) VALUES ('CANDIDATO TESTE', 'TESTE VEREADOR')",
            [],
        )?;
        let pol_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, sigla_partido, uf)
             VALUES (?1, 2024, '13', '22', 'SP')",
            [pol_id],
        )?;

        // Re-executa as migrações (ou script da migração 12)
        conn.execute("UPDATE candidaturas SET cargo = 'VEREADOR' WHERE cargo = '13'", [])?;
        conn.execute("UPDATE candidaturas SET sigla_partido = 'PL' WHERE sigla_partido = '22'", [])?;

        let (cargo, partido): (String, String) = conn.query_row(
            "SELECT cargo, sigla_partido FROM candidaturas WHERE politico_id = ?1",
            [pol_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        assert_eq!(cargo, "VEREADOR");
        assert_eq!(partido, "PL");

        Ok(())
    }

    #[test]
    fn test_migrations_dedup_despesas_parlamentares() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;
        run_migrations(&mut conn)?;

        // Insere despesa parlamentar
        conn.execute(
            "INSERT INTO despesas_parlamentares (
                casa_legislativa, parlamentar_nome, data_emissao, categoria_despesa,
                fornecedor_nome, fornecedor_cnpj_cpf, valor_liquido, numero_documento
             ) VALUES ('CAMARA', 'DEPUTADO A', '2024-05-01', 'COMBUSTIVEL', 'POSTO 1', '11111111000100', 200.0, 'NF10')",
            [],
        )?;

        // Tentativa de duplicata deve falhar ou ser ignorada pelo índice único
        let res = conn.execute(
            "INSERT INTO despesas_parlamentares (
                casa_legislativa, parlamentar_nome, data_emissao, categoria_despesa,
                fornecedor_nome, fornecedor_cnpj_cpf, valor_liquido, numero_documento
             ) VALUES ('CAMARA', 'DEPUTADO A', '2024-05-01', 'COMBUSTIVEL', 'POSTO 1', '11111111000100', 200.0, 'NF10')",
            [],
        );
        assert!(res.is_err(), "Deveria falhar devido ao índice único idx_desp_parl_dedup");

        // INSERT OR IGNORE não falha e mantém contagem em 1
        let rows = conn.execute(
            "INSERT OR IGNORE INTO despesas_parlamentares (
                casa_legislativa, parlamentar_nome, data_emissao, categoria_despesa,
                fornecedor_nome, fornecedor_cnpj_cpf, valor_liquido, numero_documento
             ) VALUES ('CAMARA', 'DEPUTADO A', '2024-05-01', 'COMBUSTIVEL', 'POSTO 1', '11111111000100', 200.0, 'NF10')",
            [],
        )?;
        assert_eq!(rows, 0);

        let count: i64 = conn.query_row("SELECT count(*) FROM despesas_parlamentares", [], |r| r.get(0))?;
        assert_eq!(count, 1);

        Ok(())
    }

    #[test]
    fn test_migrations_indices_empresas_qsa() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;
        run_migrations(&mut conn)?;

        let count_razao: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_qsa_razao'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(count_razao, 1);

        let count_socio: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_qsa_socio_nome'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(count_socio, 1);

        Ok(())
    }

    #[test]
    fn test_migrations_indices_parciais_e_enriquecimento_qsa() -> Result<()> {
        let pool = DbPool::open_in_memory()?;
        let mut conn = pool.get()?;
        run_migrations(&mut conn)?;

        let count_idx_cpf: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_politicos_cpf_valido'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(count_idx_cpf, 1, "Índice parcial idx_politicos_cpf_valido deve existir");

        let count_idx_parl: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_despesas_parlamentar_nome'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(count_idx_parl, 1, "Índice idx_despesas_parlamentar_nome deve existir");

        // Testa inserção com colunas novas de empresas_qsa
        conn.execute(
            "INSERT INTO empresas_qsa (
                cnpj_basico, cnpj_ordem, cnpj_dv, razao_social, socio_cpf_cnpj_mascarado,
                socio_nome, qualificacao_socio, data_inicio_atividade, capital_social
             ) VALUES ('12345678', '0001', '90', 'EMPRESA TESTE LTDA', '***123456**', 'SOCIO TESTE', '49', '2023-01-15', 50000.0)",
            [],
        )?;

        let cap: f64 = conn.query_row(
            "SELECT capital_social FROM empresas_qsa WHERE cnpj_basico = '12345678'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(cap, 50000.0);

        Ok(())
    }
}


