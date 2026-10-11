pub mod api;
pub mod bulk;

pub use api::{
    ApiLink, CamaraApiClient, CamaraDeputadoItem, CamaraDespesaItem, CamaraListResponse,
};
pub use bulk::{
    baixar_ceap, extrair_e_processar_ceap_zip, ingerir_ceap_bulk_em_lotes,
    processar_ceap_buffer_ou_zip, processar_ceap_zip_em_lotes,
    processar_stream_ceap_com_delimitador, processar_stream_ceap_csv, CeapBulkRecord,
};
