pub mod batch;
pub mod connection;
pub mod error;
pub mod migrations;

pub use batch::{
    batch_insert_alertas_beneficio, batch_insert_autoridades, batch_insert_beneficios,
    batch_insert_bens_candidato, batch_insert_candidatos_tse, batch_insert_despesas,
    batch_insert_despesas_parlamentares, batch_insert_emendas_parlamentares, batch_insert_receitas,
    NovaAutoridade, NovaDespesa, NovaDespesaParlamentar, NovaEmendaParlamentar, NovaReceita,
    NovoAlertaBeneficioIndevido, NovoBemCandidato, NovoBeneficio, NovoCandidatoTse,
};
pub use connection::{
    aplicar_pragmas_ingestao, apply_pragmas, checkpoint_passivo, desativar_indices_tse,
    executar_manutencao_db, recriar_indices_tse, restaurar_pragmas_padrao, transaction_immediate,
    transaction_immediate_ref, DbPool, PooledConnection,
};
pub use error::{Result, StorageError};
pub use migrations::run_migrations;
pub use rusqlite;

pub mod jobs;
pub mod money;
pub use money::Money;

pub mod integrity;
pub mod quality;
pub use integrity::{
    buscar_arquivo_por_hash, calcular_sha256_bytes, calcular_sha256_file, calcular_sha256_reader,
    listar_arquivos_ingeridos, registrar_arquivo_ingerido, verificar_retificacao,
    NovoArquivoIngerido, RegistroArquivoIngerido,
};
pub use quality::{avaliacao_politico, AvaliacaoPolitico};

pub mod ceap;
pub use ceap::{identificador_ceap, upsert_despesa_parlamentar};
