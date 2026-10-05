pub mod combustivel;
pub mod error;

pub use combustivel::{
    auditar_abastecimento, auditar_lote_abastecimentos, calcular_litros, Abastecimento,
    AlertaCombustivel, LIMITE_TANQUE_VEICULO_LEVE, PRECO_PADRAO_GASOLINA_ANP,
};
pub use error::{AuditorError, Result};
