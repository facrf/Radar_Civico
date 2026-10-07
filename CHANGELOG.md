# Changelog

Todas as alterações relevantes neste projeto são documentadas neste arquivo.

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
