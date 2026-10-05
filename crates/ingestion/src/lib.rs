pub mod error;
pub mod normalizer;

pub use error::{IngestionError, Result};
pub use normalizer::{
    converter_latin1_para_utf8, extrair_cnpj_raiz, limpar_apenas_digitos, limpar_cnpj, mascarar_cpf,
};
