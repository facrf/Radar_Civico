# Changelog

Todas as alterações relevantes neste projeto são documentadas neste arquivo.

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
