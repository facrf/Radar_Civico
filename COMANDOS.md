# COMANDOS.MD - Plano de Execução Sequencial (Radar Cívico)

Diretrizes para o agente autônomo (YOLO Mode):
1. Localize a primeira tarefa marcada como `[ ]`.
2. Modifique ou crie estritamente os arquivos descritos na tarefa.
3. Não utilize stubs com `todo!()` ou `unimplemented!()`.
4. Execute o comando de validação indicado. Se houver falha, corrija antes de avançar.
5. Marque a tarefa como `[x]`, execute o commit atômico sugerido e encerre o turno para zerar o contexto.

---

## Fase 1: Setup do Workspace e Infraestrutura Docker

- [x] 001. Criar `Cargo.toml` raiz configurando o Cargo Workspace com os membros `crates/storage`, `crates/ingestion`, `crates/auditor`, `crates/graph` e `crates/server`[cite: 1].  
  *Validação:* `cargo check`  
  *Commit:* `chore: inicializa cargo workspace com crates modulares`

- [ ] 002. Criar `Dockerfile` multi-stage (builder em Debian/Rust Slim e runner com `ca-certificates` e SQLite) e `docker-compose.yml` mapeando o volume persistente `./data:/app/data` e a porta `8080`[cite: 1].  
  *Validação:* `docker compose config`  
  *Commit:* `chore(infra): configura dockerfile multi-stage e docker-compose`

---

## Fase 2: Storage & Mini Data Lake SQLite (`crates/storage`)

- [ ] 003. Criar `crates/storage/Cargo.toml` com `rusqlite` (features `bundled`, `blob`), `tokio`, `thiserror`, `serde` e `serde_json`[cite: 1].  
  *Validação:* `cargo check -p storage`  
  *Commit:* `feat(storage): define dependencias basicas da crate storage`

- [ ] 004. Implementar `crates/storage/src/connection.rs` com pool de conexões aplicando os PRAGMAs obrigatórios: `WAL`, `synchronous = NORMAL`, `cache_size = -64000`, `temp_store = MEMORY` e `mmap_size = 30000000000`[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): implementa conexao sqlite com pragmas de alta performance`

- [ ] 005. Criar `crates/storage/src/migrations/mod.rs` e implementar migração para criação das tabelas `politicos` (com `foto_blob` binário), `candidaturas` e `bens_candidato`[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): adiciona tabelas politicos, candidaturas e bens`

- [ ] 006. Implementar migração para as tabelas `receitas_campanha`, `despesas_campanha` e `empresas_qsa` (Quadro de Sócios e Administradores)[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): adiciona tabelas de receitas, despesas e qsa da receita`

- [ ] 007. Implementar migração para as tabelas `despesas_parlamentares` (com campo `detalhes_litros`) e `contratos_publicos`[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): adiciona tabelas de despesas ceap/ceaps e contratos publicos`

- [ ] 008. Implementar migração para as tabelas de grafo relacional: `nos_rede` e `conexoes_rede` com índices bidirecionais (`idx_conexoes_origem`, `idx_conexoes_destino`, `idx_conexoes_busca`)[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): adiciona tabelas e indices para modelagem de grafos`

- [ ] 009. Implementar migração para as tabelas `cache_consultas_diario`, `registros_profissionais` (OAB/CFM/CREA) e `alertas_incompatibilidade`[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): adiciona tabelas de diários oficiais e conselhos de classe`

- [ ] 010. Implementar tabelas virtuais FTS5 (`politicos_fts` e `fornecedores_fts`) e triggers de sincronização automática[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): configura tabelas virtuais fts5 para busca textual`

- [ ] 011. Implementar métodos de inserção em lote (`batch_insert_receitas`, `batch_insert_despesas`) utilizando transações explícitas `BEGIN`/`COMMIT`[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): implementa rotinas de batch insert com transacao`

---

## Fase 3: Motor de Auditoria & Heurísticas (`crates/auditor`)

- [ ] 012. Criar `crates/auditor/Cargo.toml` com dependências para `crates/storage`, `chrono`, `serde`, `thiserror` e `anyhow`.  
  *Validação:* `cargo check -p auditor`  
  *Commit:* `feat(auditor): configura crate de auditoria e analise deterministica`

- [ ] 013. Implementar módulo `crates/auditor/src/combustivel.rs` com a heurística de capacidade volumétrica de veículos leves (> 80 litros) e cálculo de litros via preço ANP[cite: 1].  
  *Validação:* `cargo test -p auditor -- combustivel`  
  *Commit:* `feat(auditor): implementa heuristica de anomalia de combustivel do fiat uno`

- [ ] 014. Implementar módulo `crates/auditor/src/ubiquidade.rs` para detecção de despesas presenciais em cidades distantes com velocidade teórica > 800 km/h[cite: 1].  
  *Validação:* `cargo test -p auditor -- ubiquidade`  
  *Commit:* `feat(auditor): implementa detector de inconsistencia geotemporal`

- [ ] 015. Implementar módulo `crates/auditor/src/conflito_oab.rs` para verificação de incompatibilidade de secretários municipais ativos com OAB em situação "Regular" (Art. 28 da Lei 8.906/94)[cite: 1].  
  *Validação:* `cargo test -p auditor -- conflito_oab`  
  *Commit:* `feat(auditor): implementa regra do art 28 da oab para secretarios`

- [ ] 016. Implementar módulo `crates/auditor/src/triangulacao.rs` para identificar contratos diretos (PNCP) com empresas de doadores PF em até 180 dias após a posse[cite: 1].  
  *Validação:* `cargo test -p auditor -- triangulacao`  
  *Commit:* `feat(auditor): implementa detector de triangulacao doacao x contrato direto`

- [ ] 017. Implementar `crates/auditor/src/fornecedores_fantasmas.rs` para sinalizar fornecedores recém-criados (< 30 dias) ou com situação cadastral inapta na Receita[cite: 1].  
  *Validação:* `cargo test -p auditor -- fantasmas`  
  *Commit:* `feat(auditor): implementa verificador de fornecedores inaptos e recentes`

---

## Fase 4: Ingestão em Streaming & Normalização (`crates/ingestion`)

- [ ] 018. Criar `crates/ingestion/Cargo.toml` com `csv-async`, `async-zip`, `reqwest`, `tokio`, `crates/storage`, `encoding_rs` e `serde`[cite: 1].  
  *Validação:* `cargo check -p ingestion`  
  *Commit:* `feat(ingestion): configura crate de ingestao assincrona`

- [ ] 019. Implementar `crates/ingestion/src/normalizer.rs` com funções para mascaramento de CPF, limpeza de raiz de CNPJ e conversão de encoding ISO-8859-1 para UTF-8[cite: 1].  
  *Validação:* `cargo test -p ingestion -- normalizer`  
  *Commit:* `feat(ingestion): implementa normalizacao de documentos e encodings`

- [ ] 020. Implementar `crates/ingestion/src/tse_streaming.rs` para descompactar e processar streams de CSVs do TSE (`consulta_cand` e `receitas_candidatos`) sem carregar arquivos na RAM[cite: 1].  
  *Validação:* `cargo test -p ingestion -- tse`  
  *Commit:* `feat(ingestion): implementa parser streaming de receitas do tse`

- [ ] 021. Implementar parser para os arquivos de prestação de contas de despesas eleitorais (`despesas_candidatos`) do TSE[cite: 1].  
  *Validação:* `cargo test -p ingestion -- despesas_tse`  
  *Commit:* `feat(ingestion): implementa parser de despesas eleitorais`

- [ ] 022. Implementar consumidor da API de Dados Abertos da Câmara dos Deputados para extração dos registros anuais da CEAP[cite: 1].  
  *Validação:* `cargo test -p ingestion -- ceap`  
  *Commit:* `feat(ingestion): adiciona integracao com dados abertos da camara para ceap`

- [ ] 023. Implementar cliente assíncrono para a API aberta do PNCP (Portal Nacional de Contratações Públicas) para busca de contratos por CNPJ[cite: 1].  
  *Validação:* `cargo test -p ingestion -- pncp`  
  *Commit:* `feat(ingestion): implementa extrator de contratos do pncp`

- [ ] 024. Implementar cliente sob demanda para o Querido Diário com busca restrita por doador, termos de nomeação e município[cite: 1].  
  *Validação:* `cargo test -p ingestion -- querido_diario`  
  *Commit:* `feat(ingestion): implementa consulta on demand do querido diario`

- [ ] 025. Implementar scraper sob demanda para consulta de status no Cadastro Nacional dos Advogados (CNA/OAB) salvando payload em cache[cite: 1].  
  *Validação:* `cargo test -p ingestion -- oab`  
  *Commit:* `feat(ingestion): implementa consulta sob demanda ao cna da oab`

---

## Fase 5: Motor de Grafos e Métricas de Rede (`crates/graph`)

- [ ] 026. Criar `crates/graph/Cargo.toml` adicionando `petgraph`, `crates/storage`, `serde` e `thiserror`[cite: 1].  
  *Validação:* `cargo check -p graph`  
  *Commit:* `feat(graph): configura crate de modelagem e travessia de redes`

- [ ] 027. Implementar `crates/graph/src/builder.rs` para sincronizar os registros das tabelas `nos_rede` e `conexoes_rede` para um grafo direcionado em memória (`petgraph::Graph`)[cite: 1].  
  *Validação:* `cargo test -p graph -- builder`  
  *Commit:* `feat(graph): implementa construcao de grafo direcionado a partir do sqlite`

- [ ] 028. Implementar algoritmo de travessia e detecção de ciclos para rastrear conexões indiretas entre políticos e empresas em até 3 graus[cite: 1].  
  *Validação:* `cargo test -p graph -- travessia`  
  *Commit:* `feat(graph): adiciona algoritmo de busca de caminhos em ate 3 graus`

- [ ] 029. Implementar algoritmo de detecção de Fornecedor "Hub" calculando centralidade de intermediação (*Betweenness Centrality*) e concentração por coligação[cite: 1].  
  *Validação:* `cargo test -p graph -- hub`  
  *Commit:* `feat(graph): implementa algoritmo de deteccao de fornecedores hub`

---

## Fase 6: Servidor de Aplicação HTTP (`crates/server`)

- [ ] 030. Criar `crates/server/Cargo.toml` com `axum`, `tower-http` (CORS e compressão), `tokio`, `serde`, `serde_json` e as crates internas do workspace[cite: 1].  
  *Validação:* `cargo check -p server`  
  *Commit:* `feat(server): define dependencias do servidor http axum`

- [ ] 031. Implementar rotas de busca textual FTS5 (`/api/v1/busca?q=...`) retornando políticos, doadores e fornecedores em formato unificado[cite: 1].  
  *Validação:* `cargo test -p server -- busca`  
  *Commit:* `feat(server): adiciona endpoint de busca unificada via fts5`

- [ ] 032. Implementar rota de dossiê do político (`/api/v1/politico/{id}`), incluindo histórico de bens, doadores e foto em base64/blob[cite: 1].  
  *Validação:* `cargo test -p server -- politico`  
  *Commit:* `feat(server): adiciona rota de dossie consolidado do candidato`

- [ ] 033. Implementar rota do grafo de relacionamentos (`/api/v1/grafo/{id}?grau=2`) retornando nós e arestas formatados para Cytoscape/ECharts[cite: 1].  
  *Validação:* `cargo test -p server -- grafo`  
  *Commit:* `feat(server): implementa endpoint de extracao de subgrafos`

- [ ] 034. Implementar rotas do motor de auditoria (`/api/v1/auditoria/alertas`), listando anomalias ordenadas por severidade e filtros por município/ano[cite: 1].  
  *Validação:* `cargo test -p server -- alertas`  
  *Commit:* `feat(server): adiciona endpoint de listagem de anomalias detectadas`

- [ ] 035. Implementar rota sob demanda para investigação de nomeações e conselhos de classe (`/api/v1/investigar/nomeacao/{doador_id}`)[cite: 1].  
  *Validação:* `cargo test -p server -- investigar`  
  *Commit:* `feat(server): adiciona rota de disparos sob demanda de diarios e oab`

- [ ] 036. Montar o binário executável (`crates/server/src/main.rs`) que aplica migrações no arranque, instancia o pool de conexões e sobe a API com graceful shutdown[cite: 1].  
  *Validação:* `cargo run -p server -- --version`  
  *Commit:* `feat(server): consolida ponto de entrada principal e inicializacao da api`

---

## Fase 7: Frontend Web & Visualização (`web/`)

- [ ] 037. Inicializar projeto SvelteKit/Tailwind em `web/` com estrutura de componentes para painel, dossiê e busca.  
  *Validação:* `cd web && npm run check`  
  *Commit:* `feat(web): inicializa esqueleto do frontend sveltekit com tailwind`

- [ ] 038. Criar componente de dossiê do político exibindo foto oficial do TSE, cartões de métricas, histórico de patrimônio e tabela de doadores[cite: 1].  
  *Validação:* `cd web && npm run build`  
  *Commit:* `feat(web): cria componente visual de dossie do politico`

- [ ] 039. Integrar visualizador interativo de grafos (Cytoscape.js) colorindo nós por categoria e destacando arestas com anomalia[cite: 1].  
  *Validação:* `cd web && npm run build`  
  *Commit:* `feat(web): implementa componente interativo de grafos com cytoscape`

- [ ] 040. Criar painel de alertas do auditor exibindo discrepâncias de combustível, incompatibilidade de OAB e contratos pós-eleição[cite: 1].  
  *Validação:* `cd web && npm run build`  
  *Commit:* `feat(web): adiciona tela de ranking de anomalias com filtros`

---

## Fase 8: Teste de Integração Ponta a Ponta

- [ ] 041. Criar teste de integração completo em `tests/e2e_pipeline.rs`: popula dados simulados de um candidato e um doador advogado nomeado em secretaria, roda o auditor e valida o alerta[cite: 1].  
  *Validação:* `cargo test --test e2e_pipeline`  
  *Commit:* `test(e2e): valida pipeline completo de ingestao auditoria e grafo`

- [ ] 042. Validar build multi-stage final via Docker Compose e verificar subida completa de backend e banco persistido em `./data`[cite: 1].  
  *Validação:* `docker compose up --build -d && docker compose ps`  
  *Commit:* `chore(release): valida empacotamento completo em containers docker`