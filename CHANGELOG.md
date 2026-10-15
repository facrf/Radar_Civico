# Changelog

Todas as alterações relevantes neste projeto são documentadas neste arquivo.

## [Unreleased]

### Segurança e Mitigações de Vulnerabilidades

- **Autenticação Administrativa Configurável:** Implementação da estrutura `AdminAuth` permitindo controle de acesso opcional em endpoints de mutação (uploads, sincronizações, disparos de auditoria, backups e configurações) e exportação de banco de dados (`/api/v1/config/exportar/*`).
  - Habilitada definindo as variáveis de ambiente `RADAR_ADMIN_PASSWORD` (para login via interface e autenticação HTTP Basic) e/ou `RADAR_API_KEY` (para integrações automatizadas via cabeçalhos `X-API-Key` ou `Authorization: Bearer <token>`).
  - Quando nenhuma credencial estiver configurada, o sistema opera no modo local sem bloqueio, garantindo compatibilidade total com instalações existentes.
- **Sanitização de Uploads e Prevenção de Stored XSS:**
  - Validação estrita de magic bytes para imagens (PNG, JPEG, WebP e ICO) e validação estrutural contra injeções em arquivos SVG (bloqueio de tags `<script>`, `<foreignobject>`, URIs `javascript:` e manipuladores de eventos inline como `onload=`, `onerror=`).
  - Injeção obrigatória dos cabeçalhos `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'` e `X-Content-Type-Options: nosniff` nos endpoints de entrega de ícone e favicon.
- **Mitigação de SSRF e Port Scanning em Webhooks:** Restrição das conexões do cliente HTTP seguro (`public_client`) às portas convencionais de tráfego web (`80`, `443`, `8080`, `8443`), com bloqueio preventivo de endereços IP privados (RFC 1918), locais de loopback e reservados.
- **Cabeçalhos Globais de Segurança HTTP:** Middleware de segurança embutido na API Axum que anexa automaticamente em todas as respostas HTTP os cabeçalhos:
  - `X-Content-Type-Options: nosniff` (prevenção de MIME-sniffing);
  - `X-Frame-Options: SAMEORIGIN` (proteção contra clickjacking);
  - `Referrer-Policy: strict-origin-when-cross-origin`.
- **Execução do Contêiner como Usuário Não-Root:** Reestruturação do contêiner Docker para execução com privilégios reduzidos sob o usuário de sistema `radar` (UID 10001), utilizando `scripts/docker-entrypoint.sh` e `gosu` para ajuste automático e seguro de permissões no diretório `/app/data`.
- **Flexibilização de Caminhos nos Scripts:** Parametrização de caminhos absolutos nos scripts de extração e ingestão do auxílio emergencial (`scripts/extrair_auxilio_municipio.sh` e `scripts/ingerir_auxilio_completo.py`) com suporte à variável de ambiente `AUXILIO_CSV_PATH`.

### Rastreamento de Integridade e Hashes SHA-256

- **Tabela de Integridade (`arquivos_ingeridos`):** Migração 27 criando tabela para registro de checksum SHA-256, nome do arquivo, fonte de dados, quantidade de linhas ingeridas, tamanho em bytes e carimbo de data/hora.
- **Detecção de Retificações:** Identificação automática de arquivos idênticos já ingeridos e sinalização de lotes retificados, prevenindo duplicações e rastreando alterações nas fontes de dados primárias.
- **Endpoints de Verificação de Integridade:** Endpoints `GET /api/v1/config/arquivos-ingeridos` e `POST /api/v1/config/verificar-hash` para auditoria do histórico de arquivos e conferência de integridade de backups.

### Geolocalização Municipal e Busca Acentuada

- **Migração 26 (Candidaturas e Municípios do ES):** Vinculação de candidaturas municipais do Espírito Santo ao código IBGE territorial, assegurando integridade nas análises regionais.
- **Busca Tolerante a Acentuação:** Aprimoramento da busca de políticos e municípios no SQLite com normalização de acentos.

### Confiabilidade da sincronização

- **CEAP em streaming:** Download e descompactação em arquivos temporários, parsing em lotes de 500, limites de tamanho e empréstimo da conexão apenas durante cada lote; mesmo fluxo para sincronização individual, unificada e importador CEAP.
- **Resultados corretos:** Falhas de download, parsing e gravação geram `ERRO` ou `PARCIAL`, com detalhes e preservação dos lotes confirmados. O modo API exige IDs e contabiliza resultados por deputado. Consultas locais e falhas do Querido Diário não são apresentadas como novas importações bem-sucedidas.
- **Admissão de jobs:** Reserva atômica compartilhada por configuração e importadores, com `409 Conflict` para cargas conflitantes, incluindo Sincronizar Tudo e deduplicação.
- **Conclusão durável:** Fila de resultados ao lado do banco, consulta de status sem conexão do pool quando a conclusão está pendente, retentativas e recuperação no arranque. Snapshot final e histórico são gravados atomicamente, sem duplicação na reaplicação.
- **Polling:** Consultas sem sobreposição, cancelamento ao trocar job ou sair da página, intervalo crescente após falhas e indicação de persistência pendente.
- **Diagnóstico:** Logs distinguem espera por conexão, trava de transação, operação e conclusão; a duração de 15 segundos não é tratada como prova de esgotamento do pool.
- **Resolução de homônimos em auxílio emergencial:** Rejeição de homônimos com CPF mascarado que compartilham unicamente um sobrenome de alta frequência no Brasil (como Silva, Santos, Oliveira) quando o primeiro nome for divergente, alinhando a regra aos critérios estritos de identidade e eliminando falsos positivos graves.
- **Resiliência contra envenenamento de trava (`RwLock`):** Tratamento de travas envenenadas com `.unwrap_or_else(|e| e.into_inner())` no endpoint de status de importação e serialização defensiva.
- **Controle de crescimento do WAL:** Criação da função `checkpoint_passivo` (`PRAGMA wal_checkpoint(PASSIVE)`) exportada pela crate `storage` e acionada após o processamento de pacotes do TSE para evitar inchaço do arquivo `-wal`.
- **Betweenness Centrality escalável no grafo:** Algoritmo de Brandes com amostragem uniforme escalada para grafos com mais de 500 nós (`calcular_betweenness_centrality_amostrado`), eliminando riscos de degradação $O(|V| \cdot |E|)$ e removendo todos os `unwrap()` inseguros na travessia.
- **Interface web moderna e não-bloqueante:** Substituição de todos os `window.alert()` por sistema visual de toasts animados com auto-dispensa na página de configurações e feedback reativo na tela de políticos.
- **Conformidade estrita de compilação e linting:** Eliminação completa de avisos do compilador e aprovação total de `cargo clippy --workspace --all-targets -- -D warnings`.

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
