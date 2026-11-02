use rmcp::{
    model::{
        GetPromptRequestParams, GetPromptResponse, GetPromptResult, JsonObject, Prompt,
        PromptArgument, PromptMessage, Role,
    },
    ErrorData as McpError,
};
use serde_json::Value;
use tracing::{info, warn};

use crate::tools::sanitizar_parametro_log;

/// Nome do template de prompt investigativo para cruzamento entre doações de campanha e contratos PNCP.
pub const PROMPT_CRUZAR_CAMPANHA_PNCP: &str = "cruzar_campanha_contratos_pncp";

/// Descrição oficial do prompt `cruzar_campanha_contratos_pncp`.
pub const DESCRICAO_CRUZAR_CAMPANHA_PNCP: &str = "Roteiro investigativo para verificar se doadores de campanha de um político possuem contratos vigentes no PNCP com a prefeitura/órgão vinculado.";

/// Registro centralizado de templates de Prompts nativos expostos pelo `radar-mcp`.
#[derive(Debug, Default, Clone)]
pub struct PromptRegistry;

impl PromptRegistry {
    /// Cria uma nova instância do registro de prompts nativos.
    pub fn new() -> Self {
        Self
    }

    /// Retorna todos os templates de prompts configurados para o método `prompts/list`.
    pub fn list_prompts(&self) -> Vec<Prompt> {
        vec![definicao_prompt_cruzar_campanha_pncp()]
    }

    /// Monta e retorna o prompt solicitado pelo método `prompts/get`.
    #[tracing::instrument(
        name = "mcp.prompts.get",
        skip(self, request),
        fields(prompt_name = %request.name)
    )]
    pub fn get_prompt(
        &self,
        request: GetPromptRequestParams,
    ) -> Result<GetPromptResponse, McpError> {
        if request.name != PROMPT_CRUZAR_CAMPANHA_PNCP {
            warn!(
                prompt_name = %request.name,
                "Tentativa de recuperação de prompt inexistente"
            );
            return Err(McpError::invalid_params(
                format!(
                    "Prompt não encontrado: '{}'. Prompts disponíveis: ['{}']",
                    request.name, PROMPT_CRUZAR_CAMPANHA_PNCP
                ),
                None,
            ));
        }

        let args = request.arguments.unwrap_or_default();
        let (nome_ou_id_politico, municipio_uf) = extrair_argumentos_cruzar_campanha(&args)?;

        let alvo_seguro = sanitizar_parametro_log(&nome_ou_id_politico);
        let escopo_seguro = municipio_uf
            .as_deref()
            .map(sanitizar_parametro_log)
            .unwrap_or_else(|| "não especificado".to_string());

        info!(
            prompt_name = PROMPT_CRUZAR_CAMPANHA_PNCP,
            nome_ou_id_politico = %alvo_seguro,
            municipio_uf = %escopo_seguro,
            "Montando roteiro investigativo MCP 'cruzar_campanha_contratos_pncp'"
        );

        let conteudo =
            montar_roteiro_cruzar_campanha_pncp(&nome_ou_id_politico, municipio_uf.as_deref());

        let result = GetPromptResult::new(vec![PromptMessage::new_text(Role::User, conteudo)])
            .with_description(DESCRICAO_CRUZAR_CAMPANHA_PNCP);

        Ok(GetPromptResponse::from(result))
    }
}

/// Constrói a definição estruturada do template `cruzar_campanha_contratos_pncp`.
pub fn definicao_prompt_cruzar_campanha_pncp() -> Prompt {
    let argumentos = vec![
        PromptArgument::new("nome_ou_id_politico")
            .with_description(
                "Nome eleitoral, nome civil ou identificador numérico/sequencial do político a ser investigado.",
            )
            .with_required(true),
        PromptArgument::new("municipio_uf")
            .with_description(
                "Município e/ou UF da prefeitura ou órgão público vinculado (opcional, ex.: 'São Paulo/SP' ou 'SP').",
            )
            .with_required(false),
    ];

    Prompt::new(
        PROMPT_CRUZAR_CAMPANHA_PNCP,
        Some(DESCRICAO_CRUZAR_CAMPANHA_PNCP),
        Some(argumentos),
    )
}

fn extrair_argumentos_cruzar_campanha(
    args: &JsonObject,
) -> Result<(String, Option<String>), McpError> {
    let nome_ou_id_politico = args
        .get("nome_ou_id_politico")
        .and_then(|v| match v {
            Value::String(s) => Some(s.trim().to_string()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            warn!("Argumento obrigatório 'nome_ou_id_politico' ausente em prompts/get");
            McpError::invalid_params(
                "O argumento obrigatório 'nome_ou_id_politico' deve ser informado e não pode ser vazio.",
                None,
            )
        })?;

    let municipio_uf = args
        .get("municipio_uf")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok((nome_ou_id_politico, municipio_uf))
}

/// Monta o prompt de instrução encadeando os 5 passos investigativos obrigatórios.
pub fn montar_roteiro_cruzar_campanha_pncp(
    nome_ou_id_politico: &str,
    municipio_uf: Option<&str>,
) -> String {
    let recorte_territorial = match municipio_uf {
        Some(loc) => format!(
            "Recorte territorial / prefeitura ou órgão vinculado informado: **{loc}**."
        ),
        None => "Recorte territorial / prefeitura ou órgão vinculado: **identificar automaticamente a partir das candidaturas e vínculos do político**.".to_string(),
    };

    format!(
        "Você está conduzindo uma auditoria cívica assistida no Radar Cívico.\n\
Alvo da verificação: **{nome_ou_id_politico}**\n\
{recorte_territorial}\n\n\
Execute rigorosamente o encadeamento de ferramentas MCP na ordem abaixo:\n\n\
**Passo 1: Chamar `buscar` ou `detalhe_politico` para obter identificador e candidaturas.**\n\
- Se `{nome_ou_id_politico}` for um nome ou termo, execute `buscar` (ou `listar_politicos` se aplicável ao filtro territorial) para localizar o `id` exato e, em seguida, chame `detalhe_politico` com o `id` localizado.\n\
- Se já for um identificador numérico ou sequencial, chame diretamente `detalhe_politico` para recuperar o histórico de candidaturas, cargos vinculados e resumo financeiro.\n\n\
**Passo 2: Extrair a lista de doadores de campanha (usando os dados do dossiê).**\n\
- A partir do retorno de `detalhe_politico` (campo `principais_doadores`) e, quando necessário para pessoas físicas doadoras/sócias, de `dossie_cpf` ou `subgrafo`, consolide os doadores de campanha e identifique as empresas ou sócios relacionados.\n\n\
**Passo 3: Mapear os CNPJs dessas empresas via `dossie_cnpj`.**\n\
- Para cada pessoa jurídica doadora ou empresa vinculada ao quadro societário dos doadores identificados no Passo 2, execute `dossie_cnpj` informando o respectivo CNPJ para levantar a razão social, situação cadastral, quadro societário (QSA) e montantes contratados no PNCP/CEAP.\n\n\
**Passo 4: Consultar contratos/vínculos e disparar `listar_alertas` para cruzamento de indícios.**\n\
- Verifique os contratos públicos no PNCP e as conexões societárias imediatas via `subgrafo`.\n\
- Dispare `listar_alertas` (filtrando por tipos correlatos como `doador_fornecedor` / `triangulacao` ou severidade aplicável) para cruzar indícios automatizados envolvendo o político, a prefeitura/órgão vinculado e as empresas mapeadas.\n\n\
**Passo 5: Formatar relatório com links/fontes originais e avisos de heurística.**\n\
- Apresente um relatório estruturado contendo: identificação do político e mandato/órgão, tabela de doadores de campanha versus contratos no PNCP (valores em BRL e datas/anos), vínculos societários encontrados e fontes públicas primárias (TSE, PNCP, QSA/Receita Federal, CEAP).\n\
- **AVISO OBRIGATÓRIO:** Destaque explicitamente no relatório que todos os alertas e cruzamentos constituem apontamentos heurísticos para triagem investigativa sobre dados públicos e não configuram comprovação de irregularidade, acusação formal ou condenação."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_list_prompts_retorna_template_cruzar_campanha_contratos_pncp() {
        let registry = PromptRegistry::new();
        let prompts = registry.list_prompts();
        assert_eq!(prompts.len(), 1);

        let p = &prompts[0];
        assert_eq!(p.name, "cruzar_campanha_contratos_pncp");
        assert_eq!(
            p.description.as_deref(),
            Some(
                "Roteiro investigativo para verificar se doadores de campanha de um político possuem contratos vigentes no PNCP com a prefeitura/órgão vinculado."
            )
        );

        let args = p.arguments.as_ref().expect("deve definir argumentos");
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name, "nome_ou_id_politico");
        assert_eq!(args[0].required, Some(true));
        assert_eq!(args[1].name, "municipio_uf");
        assert_eq!(args[1].required, Some(false));
    }

    #[test]
    fn test_get_prompt_monta_roteiro_5_passos_e_valida_obrigatoriedade() {
        let registry = PromptRegistry::new();

        let args = json!({
            "nome_ou_id_politico": "João Exemplo",
            "municipio_uf": "São Paulo/SP"
        })
        .as_object()
        .cloned()
        .unwrap();

        let req =
            GetPromptRequestParams::new("cruzar_campanha_contratos_pncp").with_arguments(args);
        let resp = registry.get_prompt(req).expect("deve gerar o prompt");
        let GetPromptResponse::Complete(result) = resp else {
            panic!("esperado GetPromptResponse::Complete");
        };

        let serialized = serde_json::to_value(&result).expect("deve serializar GetPromptResult");
        assert_eq!(
            serialized["description"],
            "Roteiro investigativo para verificar se doadores de campanha de um político possuem contratos vigentes no PNCP com a prefeitura/órgão vinculado."
        );
        let msg_text = serialized["messages"][0]["content"]["text"]
            .as_str()
            .expect("deve conter texto na primeira mensagem");

        assert!(msg_text.contains("João Exemplo"));
        assert!(msg_text.contains("São Paulo/SP"));
        assert!(msg_text.contains("Passo 1: Chamar `buscar` ou `detalhe_politico`"));
        assert!(msg_text.contains("Passo 2: Extrair a lista de doadores de campanha"));
        assert!(msg_text.contains("Passo 3: Mapear os CNPJs dessas empresas via `dossie_cnpj`"));
        assert!(
            msg_text.contains("Passo 4: Consultar contratos/vínculos e disparar `listar_alertas`")
        );
        assert!(msg_text.contains(
            "Passo 5: Formatar relatório com links/fontes originais e avisos de heurística"
        ));

        // Verifica erro caso o argumento obrigatório `nome_ou_id_politico` esteja ausente
        let req_sem_arg = GetPromptRequestParams::new("cruzar_campanha_contratos_pncp");
        assert!(registry.get_prompt(req_sem_arg).is_err());
    }
}
