# Changelog

Todas as alterações relevantes neste projeto são documentadas neste arquivo.

## [0.1.3] - 2026-10-08

### Sincronização Unificada e Proteção de Idempotência
- **Sincronizar Tudo:** Implementação do pipeline unificado nos endpoints `POST /api/v1/config/sincronizar-tudo` e `POST /api/config/sincronizar-tudo`, orquestrando a atualização sequencial de TSE, CEAP (Câmara), Sócios QSA, OAB e Motor de Auditoria.
- **Migração 14 (Idempotência CEAP):** Criação de índice único `idx_desp_parl_dedup` sobre `(casa_legislativa, parlamentar_nome, data_emissao, fornecedor_cnpj_cpf, valor_liquido, COALESCE(numero_documento, ''))` e consumo de `INSERT OR IGNORE` em `batch_insert_despesas_parlamentares`, eliminando duplicações acidentais em sincronizações subsequentes.
- **Interface e Feedback em Tempo Real:** Botão de destaque "Sincronizar Tudo" no cabeçalho e na seção de ingestão de `/configuracoes`, acompanhado de modal de confirmação/seleção de escopo com avisos de tempo estimado e proteção contra duplicidade, integrado ao monitor de progresso em background.
- **Documentação Arquitetural:** Adição de `docs/sincronizacao_e_idempotencia.md` documentando o comportamento de cada tabela, tempos médios e fluxos de dados.

### Busca Unificada por Nome de Empresários e Empresas QSA
- **Busca por Nome em `empresas_qsa`:** Habilitação da consulta por nome de sócios (`socio_nome`) e empresários/empresas (`razao_social`) na barra de busca unificada (`/api/v1/busca`), retornando itens classificados como `SOCIO` ("Sócio (QSA)") e `EMPRESA_QSA` ("Empresa (QSA)").
- **Migração 15 (Índices B-Tree de Alto Desempenho):** Adição dos índices `idx_qsa_razao` e `idx_qsa_socio_nome` sobre a tabela `empresas_qsa`, viabilizando varreduras por intervalo de prefixo (`>= ? AND < ?~`) com tempo de resposta na faixa de 3 a 5 milissegundos em base de mais de 62 milhões de registros sem sobrecarga de I/O.
- **Dossiê Analítico Integrado:** Extensão do endpoint `/api/v1/dossie/cpf/:cpf` para localizar automaticamente vínculos societários em `empresas_qsa` também por nome textual quando o identificador de CPF mascarado não for fornecido diretamente.

## [0.1.2] - 2026-10-07

### Novas Heurísticas e Motor de Auditoria (`crates/auditor` & `crates/server`)
- **Evolução Patrimonial Desproporcional (`EVOLUCAO_PATRIMONIAL`):** Heurística determinística para identificar saltos superiores a 300% com incremento absoluto superior a R$ 200.000 entre pleitos eleitorais sucessivos do mesmo candidato.
- **Doadores Incompatíveis (`DOADOR_INCOMPATIVEL`):** Cruzamento entre doadores de campanha eleitoral (`receitas_campanha`) e beneficiários de programas de vulnerabilidade social (`beneficios_emergenciais`) com doações $\ge$ R$ 1.000.
- **Cartel / Conluio em Licitações (`CONLUIO_LICITACAO`):** Detecção de sócios em comum entre fornecedores distintos contratados pelo mesmo órgão público via integração PNCP e QSA.
- **Sobrepreço de Combustível (`COMBUSTIVEL_SOBREPRECO`):** Sinalização de notas de abastecimento parlamentar na CEAP com valor por litro faturado superior a 150% do valor de referência de mercado ANP (> R$ 8,70/L).
- **Materialização e Sincronização Sob Demanda:** Novo endpoint `POST /api/auditoria/sincronizar` para disparo em lote e gravação persistente dos alertas com chaves de deduplicação idempotentes.

### Dossiê Oficial e Visualização da Evolução Patrimonial (`web/` & `crates/server`)
- **Aba de Evolução Patrimonial no Dossiê:** Gráfico cronológico de barras proporcionais, métricas de salto percentual e absoluto, além de cards de alerta analítico em `/politicos/[id]`.
- **Exportação e Impressão Oficial:** Suporte a impressão (`window.print()`) e exportação limpa em PDF formatada com folha de estilo `@media print`, fontes primárias oficiais e identificador único de integridade digital.
- **Endpoint Dedicado:** `GET /api/politicos/:id/evolucao-patrimonial` para consumo da série temporal calculada com comparativos pleito a pleito.

### Resiliência, Backups a Quente e Webhooks
- **Snapshot Online Hot SQLite:** Endpoints `POST /api/v1/config/backup`, `GET /api/v1/config/backups` e download direto em stream sem travar escritas ativas utilizando a API nativa de backup do SQLite.
- **Interface de Gestão de Backups:** Painel de snapshots com listagem, tamanho formatado e download na tela de configurações.
- **Disparo de Webhooks de Teste:** Endpoint `POST /api/v1/config/webhook/test` para integração e envio assíncrono com feedback de status HTTP para Discord, Slack e automações externas.

## [0.1.1] - 2026-10-07

### Correções e Melhorias de Concorrência
- **storage:** Implementação de `transaction_immediate` com suporte a retentativas transparentes com backoff para mitigar contenção `SQLITE_LOCKED_SHAREDCACHE` (código 262) sob alta carga simultânea de escrita.
- **storage:** Configuração obrigatória de `PRAGMA busy_timeout = 15000;` e `PRAGMA foreign_keys = ON;` aplicada a todas as conexões criadas pelo pool e nos pragmas de ingestão.
- **storage / ingestion / server:** Padronização do uso de transações imediatas em todos os sinks de batch, migrações e rotas de importação de dados.

### Deduplicação e Consistência Relacional
- **server:** Correção na fusão transacional de políticos duplicados, garantindo a migração integral da tabela `despesas_campanha` para evitar violações de chave estrangeira.
- **server:** Recálculo automático de `total_bens_declarados` após mesclagem de patrimônios de candidatos.
- **server:** Preservação de dados cadastrais faltantes no registro canônico (`cpf_mascarado`, `data_nascimento`, `ocupacao`, `grau_instrucao`, `sq_candidato`).

### Auditoria e Heurísticas
- **auditor:** Refinamento do cruzamento de Auxílio Emergencial para rejeitar CPFs vazios e evitar falsos positivos entre homônimos com mesma máscara de CPF.
- **auditor:** Higienização de cargos e títulos honoríficos na normalização de nomes para análise comparativa.

### Frontend e Visualização
- **web:** Correção na formatação de datas em `MapaDespesas` e na página de detalhes do político para suportar strings no padrão ISO-8601 com horário (`YYYY-MM-DDTHH:MM:SS`) sem corrupção visual.
