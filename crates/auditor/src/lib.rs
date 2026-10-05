pub mod combustivel;
pub mod error;
pub mod ubiquidade;

pub use combustivel::{
    auditar_abastecimento, auditar_lote_abastecimentos, calcular_litros, Abastecimento,
    AlertaCombustivel, LIMITE_TANQUE_VEICULO_LEVE, PRECO_PADRAO_GASOLINA_ANP,
};
pub use error::{AuditorError, Result};
pub use ubiquidade::{
    auditar_ubiquidade, calcular_distancia_km, AlertaUbiquidade, DespesaPresencial,
    LIMITE_VELOCIDADE_KMH,
};
