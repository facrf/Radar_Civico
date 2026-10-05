pub mod auxilio_emergencial;
pub mod camara;
pub mod ceap;
pub mod despesas_tse;
pub mod error;
pub mod normalizer;
pub mod oab;
pub mod pncp;
pub mod querido_diario;
pub mod tse_streaming;

pub use auxilio_emergencial::{
    ingerir_auxilio_emergencial_em_lotes, processar_stream_auxilio_com_delimitador,
    processar_stream_auxilio_emergencial, BeneficioCsvRecord,
};
pub use camara::{
    extrair_e_processar_ceap_zip, ingerir_ceap_bulk_em_lotes, processar_ceap_buffer_ou_zip,
    processar_stream_ceap_com_delimitador, processar_stream_ceap_csv, CamaraApiClient,
    CamaraDeputadoItem, CamaraDespesaItem, CeapBulkRecord,
};
pub use ceap::{CeapApiResponse, CeapClient, CeapItemApi};
pub use despesas_tse::{
    ingerir_despesas_tse_em_lotes, processar_stream_despesas_tse, DespesaTseCsvRecord,
};
pub use error::{IngestionError, Result};
pub use normalizer::{
    converter_latin1_para_utf8, extrair_cnpj_raiz, limpar_apenas_digitos, limpar_cnpj, mascarar_cpf,
};
pub use oab::{CnaItemPayload, OabConsultaResult, OabScraperClient};
pub use pncp::{PncpClient, PncpConsultaResponse, PncpContratoItem, PncpOrgao};
pub use querido_diario::{ExcerptDiario, QueridoDiarioApiResponse, QueridoDiarioClient};
pub use tse_streaming::{
    ingerir_receitas_tse_em_lotes, processar_stream_consulta_cand, processar_stream_receitas,
    CandidatoCsvRecord, ReceitaCsvRecord,
};
