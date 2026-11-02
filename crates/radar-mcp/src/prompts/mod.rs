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

/// Nome do template de prompt investigativo para auditoria de fornecedor de cota parlamentar (CEAP).
pub const PROMPT_AUDITAR_FORNECEDOR_CEAP: &str = "auditar_fornecedor_ceap";

/// Descrição oficial do prompt `auditar_fornecedor_ceap`.
pub const DESCRICAO_AUDITAR_FORNECEDOR_CEAP: &str = "Roteiro investigativo para auditar fornecedor de cota parlamentar (CEAP), avaliando situação cadastral, quadro societário, concentração de repasses e anomalias de abastecimento (> 80L).";

/// Nome do template de prompt investigativo para mapeamento de rede societária em múltiplos saltos.
pub const PROMPT_MAPEAR_REDE_SOCIETARIA: &str = "mapear_rede_societaria";

/// Descrição oficial do prompt `mapear_rede_societaria`.
pub const DESCRICAO_MAPEAR_REDE_SOCIETARIA: &str = "Roteiro investigativo para mapear conexões societárias em até 2 saltos a partir de um CPF, CNPJ ou ID de político, cruzando sócios em comum com contratos públicos e alertas.";

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
        vec![
            definicao_prompt_cruzar_campanha_pncp(),
            definicao_prompt_auditar_fornecedor_ceap(),
            definicao_prompt_mapear_rede_societaria(),
        ]
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
        let args = request.arguments.unwrap_or_default();

        match request.name.as_str() {
            PROMPT_CRUZAR_CAMPANHA_PNCP => {
                let (nome_ou_id_politico, municipio_uf) =
                    extrair_argumentos_cruzar_campanha(&args)?;

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

                let conteudo = montar_roteiro_cruzar_campanha_pncp(
                    &nome_ou_id_politico,
                    municipio_uf.as_deref(),
                );

                let result =
                    GetPromptResult::new(vec![PromptMessage::new_text(Role::User, conteudo)])
                        .with_description(DESCRICAO_CRUZAR_CAMPANHA_PNCP);

                Ok(GetPromptResponse::from(result))
            }
            PROMPT_AUDITAR_FORNECEDOR_CEAP => {
                let (cnpj, uf) = extrair_argumentos_auditar_fornecedor(&args)?;
                let cnpj_seguro = sanitizar_parametro_log(&cnpj);
                let uf_segura = uf
                    .as_deref()
                    .map(sanitizar_parametro_log)
                    .unwrap_or_else(|| "todas".to_string());

                info!(
                    prompt_name = PROMPT_AUDITAR_FORNECEDOR_CEAP,
                    cnpj = %cnpj_seguro,
                    uf = %uf_segura,
                    "Montando roteiro investigativo MCP 'auditar_fornecedor_ceap'"
                );

                let conteudo = montar_roteiro_auditar_fornecedor_ceap(&cnpj, uf.as_deref());
                let result =
                    GetPromptResult::new(vec![PromptMessage::new_text(Role::User, conteudo)])
                        .with_description(DESCRICAO_AUDITAR_FORNECEDOR_CEAP);

                Ok(GetPromptResponse::from(result))
            }
            PROMPT_MAPEAR_REDE_SOCIETARIA => {
                let identificador = extrair_argumento_identificador_rede(&args)?;
                let id_seguro = sanitizar_parametro_log(&identificador);

                info!(
                    prompt_name = PROMPT_MAPEAR_REDE_SOCIETARIA,
                    identificador = %id_seguro,
                    "Montando roteiro investigativo MCP 'mapear_rede_societaria'"
                );

                let conteudo = montar_roteiro_mapear_rede_societaria(&identificador);
                let result =
                    GetPromptResult::new(vec![PromptMessage::new_text(Role::User, conteudo)])
                        .with_description(DESCRICAO_MAPEAR_REDE_SOCIETARIA);

                Ok(GetPromptResponse::from(result))
            }
            _ => {
                warn!(
                    prompt_name = %request.name,
                    "Tentativa de recuperação de prompt inexistente"
                );
                Err(McpError::invalid_params(
                    format!(
                        "Prompt não encontrado: '{}'. Prompts disponíveis: ['{}', '{}', '{}']",
                        request.name,
                        PROMPT_CRUZAR_CAMPANHA_PNCP,
                        PROMPT_AUDITAR_FORNECEDOR_CEAP,
                        PROMPT_MAPEAR_REDE_SOCIETARIA
                    ),
                    None,
                ))
            }
        }
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

/// Constrói a definição estruturada do template `auditar_fornecedor_ceap`.
pub fn definicao_prompt_auditar_fornecedor_ceap() -> Prompt {
    let argumentos = vec![
        PromptArgument::new("cnpj")
            .with_description(
                "CNPJ do fornecedor da Cota para o Exercício da Atividade Parlamentar (14 dígitos, com ou sem pontuação).",
            )
            .with_required(true),
        PromptArgument::new("uf")
            .with_description(
                "Sigla da UF para filtrar parlamentares ou escopo regional (opcional, ex.: 'SP', 'DF').",
            )
            .with_required(false),
    ];

    Prompt::new(
        PROMPT_AUDITAR_FORNECEDOR_CEAP,
        Some(DESCRICAO_AUDITAR_FORNECEDOR_CEAP),
        Some(argumentos),
    )
}

/// Constrói a definição estruturada do template `mapear_rede_societaria`.
pub fn definicao_prompt_mapear_rede_societaria() -> Prompt {
    let argumentos = vec![
        PromptArgument::new("identificador")
            .with_description(
                "CPF, CNPJ ou identificador de político (`cnpj:...`, `cpf:...`, `pol:...` ou documento bruto) para iniciar a expansão da rede societária.",
            )
            .with_required(true),
    ];

    Prompt::new(
        PROMPT_MAPEAR_REDE_SOCIETARIA,
        Some(DESCRICAO_MAPEAR_REDE_SOCIETARIA),
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

fn extrair_argumentos_auditar_fornecedor(
    args: &JsonObject,
) -> Result<(String, Option<String>), McpError> {
    let cnpj = args
        .get("cnpj")
        .and_then(|v| match v {
            Value::String(s) => Some(s.trim().to_string()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            warn!(
                "Argumento obrigatório 'cnpj' ausente em prompts/get para auditar_fornecedor_ceap"
            );
            McpError::invalid_params(
                "O argumento obrigatório 'cnpj' deve ser informado e não pode ser vazio.",
                None,
            )
        })?;

    let uf = args
        .get("uf")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty());

    Ok((cnpj, uf))
}

fn extrair_argumento_identificador_rede(args: &JsonObject) -> Result<String, McpError> {
    args.get("identificador")
        .and_then(|v| match v {
            Value::String(s) => Some(s.trim().to_string()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            warn!("Argumento obrigatório 'identificador' ausente em prompts/get para mapear_rede_societaria");
            McpError::invalid_params(
                "O argumento obrigatório 'identificador' deve ser informado e não pode ser vazio.",
                None,
            )
        })
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

/// Monta o roteiro investigativo para auditoria de fornecedor de cota parlamentar (CEAP).
pub fn montar_roteiro_auditar_fornecedor_ceap(cnpj: &str, uf: Option<&str>) -> String {
    let filtro_uf = match uf {
        Some(sigla) => format!("Escopo regional de interesse: **{sigla}**."),
        None => "Escopo regional: **nacional (todas as UFs)**.".to_string(),
    };

    format!(
        "Você está conduzindo uma auditoria de fornecedor de cota parlamentar (CEAP) no Radar Cívico.\n\
Fornecedor alvo (CNPJ): **{cnpj}**\n\
{filtro_uf}\n\n\
Execute o seguinte roteiro técnico utilizando as ferramentas MCP:\n\n\
**Passo 1: Consultar o dossiê cadastral e financeiro via `dossie_cnpj`.**\n\
- Chame `dossie_cnpj` para `{cnpj}` e verifique: razão social, situação cadastral na Receita Federal (atenção para empresas baixadas, inaptas ou suspensas), data de abertura frente às datas de faturamento, capital social/porte e volume total faturado na CEAP e no PNCP.\n\n\
**Passo 2: Mapear conexões diretas com gabinetes parlamentares via `subgrafo`.**\n\
- Chame `subgrafo` com `id: \"cnpj:{cnpj}\"` (apenas dígitos no CNPJ) para identificar quais parlamentares ou órgãos concentram pagamentos para esta pessoa jurídica e se há concentração atípica de repasses (`FORNECEDOR_HUB`).\n\n\
**Passo 3: Auditar o Quadro de Sócios e Administradores (QSA).**\n\
- A partir dos sócios listados no `dossie_cnpj`, verifique se possuem outras empresas fornecedoras do Estado ou se realizaram doações de campanha para os parlamentares clientes (consultando `dossie_cpf` quando o CPF estiver disponível).\n\n\
**Passo 4: Cruzar anomalias determinísticas via `listar_alertas`.**\n\
- Consulte `listar_alertas` para verificar indícios de superfaturamento ou abastecimento acima do limite físico de tanque de veículos leves (`> 80 litros`), notas emitidas por empresa inativa ou concentração de repasses.\n\n\
**Passo 5: Consolidar parecer técnico com fontes primárias.**\n\
- Apresente valores em BRL (`R$`), datas no formato `DD/MM/AAAA`, cite as fontes oficiais (Câmara dos Deputados / Senado / Receita Federal / TSE) e inclua o aviso obrigatório de que os apontamentos são heurísticos para triagem investigativa."
    )
}

/// Monta o roteiro investigativo para mapeamento de rede societária em múltiplos saltos.
pub fn montar_roteiro_mapear_rede_societaria(identificador: &str) -> String {
    format!(
        "Você está conduzindo um mapeamento de rede societária e vínculos públicos no Radar Cívico.\n\
Nó inicial da investigação: **{identificador}**\n\n\
Execute a expansão da rede em múltiplos saltos utilizando as ferramentas MCP:\n\n\
**Passo 1: Identificar e qualificar o nó raiz.**\n\
- Se `{identificador}` for um CNPJ, chame `dossie_cnpj`. Se for um CPF, chame `dossie_cpf`. Se for um político ou nome, chame `buscar` e `detalhe_politico`.\n\n\
**Passo 2: Expandir conexões de 1º grau (Raio 1) via `subgrafo`.**\n\
- Chame `subgrafo` para o identificador canônico (`cnpj:...`, `cpf:...` ou `pol:...`) e liste todas as empresas, sócios, doadores e órgãos públicos diretamente conectados.\n\n\
**Passo 3: Expandir conexões de 2º grau (empresas-irmãs e sócios em comum).**\n\
- Para os principais sócios ou empresas conectados no 1º grau, consulte `dossie_cnpj` / `dossie_cpf` ou `subgrafo` adicional para identificar empresas-irmãs que compartilhem o mesmo quadro societário e celebrem contratos no PNCP ou recebam recursos da CEAP.\n\n\
**Passo 4: Verificar alertas de conflito de interesses e triangulação via `listar_alertas`.**\n\
- Verifique se algum integrante da rede consta em alertas de `doador_fornecedor`, `triangulacao` ou impedimento profissional (ex.: Art. 28 da Lei 8.906/94 — OAB).\n\n\
**Passo 5: Apresentar matriz de vínculos e nota metodológica.**\n\
- Estruture um mapa textual das conexões (`Entidade A --[vínculo]--> Entidade B`), valores envolvidos em BRL, fontes oficiais e o aviso explícito de que os cruzamentos constituem triagem heurística e não prova de ilicitude."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_list_prompts_retorna_todos_os_3_templates_investigativos() {
        let registry = PromptRegistry::new();
        let prompts = registry.list_prompts();
        assert_eq!(prompts.len(), 3);

        let p0 = &prompts[0];
        assert_eq!(p0.name, "cruzar_campanha_contratos_pncp");
        assert_eq!(
            p0.description.as_deref(),
            Some(DESCRICAO_CRUZAR_CAMPANHA_PNCP)
        );
        let args0 = p0.arguments.as_ref().expect("deve definir argumentos");
        assert_eq!(args0.len(), 2);
        assert_eq!(args0[0].name, "nome_ou_id_politico");
        assert_eq!(args0[0].required, Some(true));
        assert_eq!(args0[1].name, "municipio_uf");
        assert_eq!(args0[1].required, Some(false));

        let p1 = &prompts[1];
        assert_eq!(p1.name, "auditar_fornecedor_ceap");
        assert_eq!(
            p1.description.as_deref(),
            Some(DESCRICAO_AUDITAR_FORNECEDOR_CEAP)
        );
        let args1 = p1.arguments.as_ref().expect("deve definir argumentos");
        assert_eq!(args1.len(), 2);
        assert_eq!(args1[0].name, "cnpj");
        assert_eq!(args1[0].required, Some(true));
        assert_eq!(args1[1].name, "uf");
        assert_eq!(args1[1].required, Some(false));

        let p2 = &prompts[2];
        assert_eq!(p2.name, "mapear_rede_societaria");
        assert_eq!(
            p2.description.as_deref(),
            Some(DESCRICAO_MAPEAR_REDE_SOCIETARIA)
        );
        let args2 = p2.arguments.as_ref().expect("deve definir argumentos");
        assert_eq!(args2.len(), 1);
        assert_eq!(args2[0].name, "identificador");
        assert_eq!(args2[0].required, Some(true));
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
        assert_eq!(serialized["description"], DESCRICAO_CRUZAR_CAMPANHA_PNCP);
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

        // Verifica os novos prompts: auditar_fornecedor_ceap e mapear_rede_societaria
        let args_ceap = json!({ "cnpj": "00.000.000/0001-91", "uf": "df" })
            .as_object()
            .cloned()
            .unwrap();
        let resp_ceap = registry
            .get_prompt(
                GetPromptRequestParams::new("auditar_fornecedor_ceap").with_arguments(args_ceap),
            )
            .expect("deve gerar prompt auditar_fornecedor_ceap");
        let GetPromptResponse::Complete(res_ceap) = resp_ceap else {
            panic!("esperado Complete");
        };
        let text_ceap = serde_json::to_value(&res_ceap).unwrap()["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text_ceap.contains("00.000.000/0001-91"));
        assert!(text_ceap.contains("DF"));
        assert!(text_ceap.contains("> 80 litros"));

        let args_rede = json!({ "identificador": "cnpj:00000000000191" })
            .as_object()
            .cloned()
            .unwrap();
        let resp_rede = registry
            .get_prompt(
                GetPromptRequestParams::new("mapear_rede_societaria").with_arguments(args_rede),
            )
            .expect("deve gerar prompt mapear_rede_societaria");
        let GetPromptResponse::Complete(res_rede) = resp_rede else {
            panic!("esperado Complete");
        };
        let text_rede = serde_json::to_value(&res_rede).unwrap()["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text_rede.contains("cnpj:00000000000191"));
        assert!(text_rede.contains("Lei 8.906/94"));

        // Verifica erro caso argumentos obrigatórios estejam ausentes
        assert!(registry
            .get_prompt(GetPromptRequestParams::new(
                "cruzar_campanha_contratos_pncp"
            ))
            .is_err());
        assert!(registry
            .get_prompt(GetPromptRequestParams::new("auditar_fornecedor_ceap"))
            .is_err());
        assert!(registry
            .get_prompt(GetPromptRequestParams::new("mapear_rede_societaria"))
            .is_err());
    }
}
