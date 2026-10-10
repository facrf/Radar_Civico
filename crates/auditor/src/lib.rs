pub mod auxilio_indevido;
pub mod capital_social_desproporcional;
pub mod cartel_licitacao;
pub mod combustivel;
pub mod conflito_oab;
pub mod doador_incompativel;
pub mod error;
pub mod evolucao_patrimonial;
pub mod fornecedores_fantasmas;
pub mod nepotismo;
pub mod score;
pub mod triangulacao;
pub mod ubiquidade;

pub use auxilio_indevido::{
    auditar_lote_auxilio_indevido, auditar_recebimento_auxilio, executar_auditoria_auxilio_sqlite,
    AlertaAuxilioIndevido, MotivoAuxilioIndevido, PoliticoPerfilAuxilio, RegistroAuxilio,
    LIMITE_BENS_AUXILIO,
};
pub use capital_social_desproporcional::{
    auditar_capital_desproporcional, AlertaCapitalDesproporcional, FornecedorCapitalFaturamento,
    LIMITE_CAPITAL_INVEROSIMIL_MAX, LIMITE_FATURAMENTO_PUBLICO_MIN,
};
pub use cartel_licitacao::{
    auditar_socios_comuns_contratos, AlertaConluioLicitacao, ContratoEmpresaSocio,
};
pub use combustivel::{
    auditar_abastecimento, auditar_abastecimento_com_limite, auditar_lote_abastecimentos,
    auditar_sobrepreco_combustivel, calcular_litros, Abastecimento, AlertaCombustivel,
    AlertaSobreprecoCombustivel, LIMITE_SOBREPRECO_PERCENTUAL, LIMITE_TANQUE_VEICULO_LEVE,
    PRECO_PADRAO_GASOLINA_ANP,
};
pub use conflito_oab::{
    auditar_conflito_oab, auditar_lote_conflitos_oab, is_cargo_incompativel, AlertaConflitoOab,
    OcupanteCargo, RegistroOab,
};
pub use doador_incompativel::{
    auditar_doador_incompativel, AlertaDoadorIncompativel, BeneficiarioSocialAnalise,
    DoadorCampanhaAnalise, LIMITE_DOACAO_SUSPEITA_BENEFICIARIO,
};
pub use error::{AuditorError, Result};
pub use evolucao_patrimonial::{
    auditar_evolucao_patrimonial, AlertaEvolucaoPatrimonial, DeclaracaoPatrimonioAno,
    LIMITE_INCREMENTO_ABSOLUTO_PADRAO, LIMITE_VARIACAO_PERCENTUAL_PADRAO,
};
pub use fornecedores_fantasmas::{
    auditar_fornecedores_fantasmas, AlertaFornecedorFantasma, FornecedorReceita, PagamentoFornecedor,
    TipoIrregularidadeFornecedor, DIAS_LIMITE_RECEM_CRIADA,
};
pub use nepotismo::{
    auditar_possivel_parentesco, extrair_sobrenomes_raros, AlertaPossivelParentesco,
    AlvoAuditoriaParentesco,
};
pub use score::{
    calcular_score_integridade, ItemPenalidadeScore, NivelRiscoCivico, ResumoScoreIntegridade,
};
pub use triangulacao::{
    auditar_triangulacao, auditar_triangulacao_com_janela, AlertaTriangulacao, ContratoPublico,
    DoadorCampanha, SocioEmpresa, JANELA_DIAS_POSSE,
};
pub use ubiquidade::{
    auditar_ubiquidade, calcular_distancia_km, AlertaUbiquidade, DespesaPresencial,
    LIMITE_VELOCIDADE_KMH,
};
