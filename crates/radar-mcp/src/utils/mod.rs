pub mod compact;

pub use compact::{
    compactar_json_para_llm, eh_chave_auditoria_interna, podar_valor_json,
    serializar_json_compacto, truncar_sublistas_json, CHAVES_AUDITORIA_INTERNA,
    DEFAULT_MAX_ITENS_SUBLISTA,
};
