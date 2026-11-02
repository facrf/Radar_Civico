use rmcp::{
    model::{
        ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
        ResourceContents,
    },
    ErrorData as McpError,
};
use serde_json::json;
use tracing::{info, warn};

use crate::utils::compact::{podar_valor_json, serializar_json_compacto};
use crate::RadarHttpClient;

/// URI do recurso estático com a metodologia e fundamentação das heurísticas de auditoria.
pub const URI_METODOLOGIA_HEURISTICAS: &str = "radar://metodologia/heuristicas";

/// URI do recurso dinâmico com o status da instância e versão do motor do Radar Cívico.
pub const URI_STATUS_FONTES: &str = "radar://status/fontes";

/// Registro centralizado de recursos (`resources/list` e `resources/read`) expostos pelo `radar-mcp`.
#[derive(Debug, Default, Clone)]
pub struct ResourceRegistry;

impl ResourceRegistry {
    /// Cria uma nova instância do registro de recursos MCP.
    pub fn new() -> Self {
        Self
    }

    /// Lista todos os recursos disponíveis para o método `resources/list`.
    pub fn list_resources(&self) -> Vec<Resource> {
        vec![
            Resource::new(URI_METODOLOGIA_HEURISTICAS, "metodologia_heuristicas")
                .with_title("Metodologia e Heurísticas Determinísticas do Radar Cívico")
                .with_description(
                    "Documentação técnica e fundamentação legal das regras determinísticas de auditoria (combustível > 80L, Art. 28 da Lei 8.906/94 - OAB, janela de 180 dias de doadores/contratos e concentração de repasses).",
                )
                .with_mime_type("application/json"),
            Resource::new(URI_STATUS_FONTES, "status_fontes")
                .with_title("Status do Motor e Fontes Públicas do Radar Cívico")
                .with_description(
                    "Status operacional da API local do Radar Cívico, versão do motor e relação de bases públicas primárias integradas.",
                )
                .with_mime_type("application/json"),
        ]
    }

    /// Lê o conteúdo de um recurso específico para o método `resources/read`.
    #[tracing::instrument(
        name = "mcp.resources.read",
        skip(self, client, request),
        fields(uri = %request.uri)
    )]
    pub async fn read_resource(
        &self,
        client: &RadarHttpClient,
        request: ReadResourceRequestParams,
    ) -> Result<ReadResourceResponse, McpError> {
        match request.uri.as_str() {
            URI_METODOLOGIA_HEURISTICAS => {
                info!(
                    uri = URI_METODOLOGIA_HEURISTICAS,
                    "Lendo recurso estático de metodologia e heurísticas"
                );
                let payload = construir_documento_metodologia();
                let texto = serializar_json_compacto(&payload);
                let contents = ResourceContents::text(texto, URI_METODOLOGIA_HEURISTICAS)
                    .with_mime_type("application/json");
                Ok(ReadResourceResponse::from(ReadResourceResult::new(vec![
                    contents,
                ])))
            }
            URI_STATUS_FONTES => {
                info!(
                    uri = URI_STATUS_FONTES,
                    "Consultando status dinâmico da instância do Radar Cívico"
                );
                let status_api = match client.get_json("/api/versao", &[]).await {
                    Ok(mut val) => {
                        podar_valor_json(&mut val);
                        json!({
                            "online": true,
                            "radar_url": client.base_url(),
                            "api_versao": val,
                        })
                    }
                    Err(_) => json!({
                        "online": false,
                        "radar_url": client.base_url(),
                        "mensagem": format!(
                            "Não foi possível conectar à API local do Radar em {}. Verifique se o serviço está ativo.",
                            client.base_url()
                        )
                    }),
                };

                let payload = json!({
                    "uri": URI_STATUS_FONTES,
                    "mcp_versao": env!("CARGO_PKG_VERSION"),
                    "instancia": status_api,
                    "fontes_publicas_suportadas": [
                        "TSE (Tribunal Superior Eleitoral — Candidaturas, Bens, Receitas e Despesas de Campanha)",
                        "Receita Federal (CNPJ e Quadro de Sócios e Administradores — QSA)",
                        "PNCP (Portal Nacional de Contratações Públicas — Contratos e Licitações)",
                        "Câmara dos Deputados e Senado Federal (CEAP / CEAPS e Emendas Parlamentares)",
                        "Portal da Transparência / CGU (Sanções CEIS/CNEP, Benefícios e Emendas)",
                        "CNA / OAB (Cadastro Nacional dos Advogados — verificação de impedimento Art. 28 da Lei 8.906/94)"
                    ]
                });

                let texto = serializar_json_compacto(&payload);
                let contents = ResourceContents::text(texto, URI_STATUS_FONTES)
                    .with_mime_type("application/json");
                Ok(ReadResourceResponse::from(ReadResourceResult::new(vec![
                    contents,
                ])))
            }
            _ => {
                warn!(uri = %request.uri, "Tentativa de leitura de recurso MCP inexistente");
                Err(McpError::invalid_params(
                    format!(
                        "Recurso não encontrado: '{}'. Recursos disponíveis: ['{}', '{}']",
                        request.uri, URI_METODOLOGIA_HEURISTICAS, URI_STATUS_FONTES
                    ),
                    None,
                ))
            }
        }
    }
}

fn construir_documento_metodologia() -> serde_json::Value {
    json!({
        "uri": URI_METODOLOGIA_HEURISTICAS,
        "natureza_juridica": "Todos os alertas gerados pelo Radar Cívico são apontamentos heurísticos determinísticos sobre bases públicas oficiais para fins de triagem investigativa e controle social. Não constituem prova de ilicitude, acusação formal ou condenação.",
        "regras_deterministicas": [
            {
                "codigo": "LIMITE_COMBUSTIVEL_CEAP",
                "titulo": "Volume de Combustível Superior ao Limite Físico (> 80 Litros)",
                "criterio": "Sinaliza notas fiscais da Cota para o Exercício da Atividade Parlamentar (CEAP) cujo volume estimado de combustível em um único abastecimento de veículo leve ultrapassa 80 litros.",
                "fontes_primarias": ["CEAP / Câmara dos Deputados", "ANP (preço médio de referência)"]
            },
            {
                "codigo": "CONFLITO_INTERESSES_OAB",
                "titulo": "Incompatibilidade no Exercício da Advocacia (Art. 28 da Lei 8.906/94)",
                "criterio": "Identifica ocupantes de cargos executivos ou secretarias municipais/estaduais que permanecem com inscrição 'Regular' ativa no Cadastro Nacional dos Advogados (CNA/OAB) sem anotação de impedimento ou licenciamento.",
                "fontes_primarias": ["Diários Oficiais / Portais de Transparência", "CNA / OAB"]
            },
            {
                "codigo": "DOADOR_FORNECEDOR_180_DIAS",
                "titulo": "Contratação Pública ou Inexigibilidade com Doador de Campanha (< 180 dias)",
                "criterio": "Sinaliza contratos públicos (especialmente inexigibilidade ou dispensa de licitação) firmados com pessoas jurídicas cujos sócios realizaram doações eleitorais ao agente político em janela inferior a 180 dias da posse ou contratação.",
                "fontes_primarias": ["TSE (Prestação de Contas Eleitorais)", "PNCP", "QSA / Receita Federal"]
            },
            {
                "codigo": "FORNECEDOR_HUB",
                "titulo": "Concentração Atípica de Repasses Partidários e Parlamentares",
                "criterio": "Calcula o grau de centralidade e concentração financeira de pessoas jurídicas que recebem repasses simultâneos de múltiplos diretórios partidários ou gabinetes parlamentares, cruzando a data de abertura e o porte da empresa.",
                "fontes_primarias": ["TSE", "CEAP / Câmara dos Deputados", "QSA / Receita Federal"]
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_resources_retorna_metodologia_e_status() {
        let registry = ResourceRegistry::new();
        let resources = registry.list_resources();
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0].uri, URI_METODOLOGIA_HEURISTICAS);
        assert_eq!(resources[1].uri, URI_STATUS_FONTES);
    }
}
