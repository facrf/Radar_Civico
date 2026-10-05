pub mod auxilio_indevido;
pub mod combustivel;
pub mod conflito_oab;
pub mod error;
pub mod fornecedores_fantasmas;
pub mod triangulacao;
pub mod ubiquidade;

pub use auxilio_indevido::{
    auditar_lote_auxilio_indevido, auditar_recebimento_auxilio, executar_auditoria_auxilio_sqlite,
    AlertaAuxilioIndevido, MotivoAuxilioIndevido, PoliticoPerfilAuxilio, RegistroAuxilio,
    LIMITE_BENS_AUXILIO,
};
pub use combustivel::{
    auditar_abastecimento, auditar_lote_abastecimentos, calcular_litros, Abastecimento,
    AlertaCombustivel, LIMITE_TANQUE_VEICULO_LEVE, PRECO_PADRAO_GASOLINA_ANP,
};
pub use conflito_oab::{
    auditar_conflito_oab, auditar_lote_conflitos_oab, is_cargo_incompativel, AlertaConflitoOab,
    OcupanteCargo, RegistroOab,
};
pub use error::{AuditorError, Result};
pub use fornecedores_fantasmas::{
    auditar_fornecedores_fantasmas, AlertaFornecedorFantasma, FornecedorReceita, PagamentoFornecedor,
    TipoIrregularidadeFornecedor, DIAS_LIMITE_RECEM_CRIADA,
};
pub use triangulacao::{
    auditar_triangulacao, AlertaTriangulacao, ContratoPublico, DoadorCampanha, SocioEmpresa,
    JANELA_DIAS_POSSE,
};
pub use ubiquidade::{
    auditar_ubiquidade, calcular_distancia_km, AlertaUbiquidade, DespesaPresencial,
    LIMITE_VELOCIDADE_KMH,
};
