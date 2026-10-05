pub mod ceap;
pub mod despesas_tse;
pub mod error;
pub mod normalizer;
pub mod pncp;
pub mod querido_diario;
pub mod tse_streaming;

pub use ceap::{CeapApiResponse, CeapClient, CeapItemApi};
pub use despesas_tse::{
    ingerir_despesas_tse_em_lotes, processar_stream_despesas_tse, DespesaTseCsvRecord,
};
pub use error::{IngestionError, Result};
pub use normalizer::{
    converter_latin1_para_utf8, extrair_cnpj_raiz, limpar_apenas_digitos, limpar_cnpj, mascarar_cpf,
};
pub use pncp::{PncpClient, PncpConsultaResponse, PncpContratoItem, PncpOrgao};
pub use querido_diario::{ExcerptDiario, QueridoDiarioApiResponse, QueridoDiarioClient};
pub use tse_streaming::{
    ingerir_receitas_tse_em_lotes, processar_stream_consulta_cand, processar_stream_receitas,
    CandidatoCsvRecord, ReceitaCsvRecord,
};
