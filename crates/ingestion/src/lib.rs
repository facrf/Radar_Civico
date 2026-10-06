pub mod auxilio_emergencial;
pub mod camara;
pub mod ceap;
pub mod despesas_tse;
pub mod error;
pub mod normalizer;
pub mod oab;
pub mod pncp;
pub mod progress;
pub mod querido_diario;
pub mod tse_ckan;
pub mod tse_streaming;

pub use progress::{
    finish_import_progress, get_import_progress, reset_import_progress, set_import_error,
    update_import_progress_batch, ImportProgress,
};

pub use tse_ckan::{
    baixar_e_processar_pacote_tse, descobrir_urls_tse, descobrir_urls_tse_com_base,
    processar_csv_tse_str, processar_csv_tse_str_com_progresso, processar_zip_tse_bytes,
    processar_zip_tse_bytes_com_progresso, selecionar_arquivos_zip, CkanPackage, CkanResource,
    CkanResponse, LOTE_BATCH_SIZE,
};

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
