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

- [x] 002. Criar `Dockerfile` multi-stage (builder em Debian/Rust Slim e runner com `ca-certificates` e SQLite) e `docker-compose.yml` mapeando o volume persistente `./data:/app/data` e a porta `8080`[cite: 1].  
  *Validação:* `docker compose config`  
  *Commit:* `chore(infra): configura dockerfile multi-stage e docker-compose`

---

## Fase 2: Storage & Mini Data Lake SQLite (`crates/storage`)

- [x] 003. Criar `crates/storage/Cargo.toml` com `rusqlite` (features `bundled`, `blob`), `tokio`, `thiserror`, `serde` e `serde_json`[cite: 1].  
  *Validação:* `cargo check -p storage`  
  *Commit:* `feat(storage): define dependencias basicas da crate storage`

- [x] 004. Implementar `crates/storage/src/connection.rs` com pool de conexões aplicando os PRAGMAs obrigatórios: `WAL`, `synchronous = NORMAL`, `cache_size = -64000`, `temp_store = MEMORY` e `mmap_size = 30000000000`[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): implementa conexao sqlite com pragmas de alta performance`

- [x] 005. Criar `crates/storage/src/migrations/mod.rs` e implementar migração para criação das tabelas `politicos` (com `foto_blob` binário), `candidaturas` e `bens_candidato`[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): adiciona tabelas politicos, candidaturas e bens`

- [x] 006. Implementar migração para as tabelas `receitas_campanha`, `despesas_campanha` e `empresas_qsa` (Quadro de Sócios e Administradores)[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): adiciona tabelas de receitas, despesas e qsa da receita`

- [x] 007. Implementar migração para as tabelas `despesas_parlamentares` (com campo `detalhes_litros`) e `contratos_publicos`[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): adiciona tabelas de despesas ceap/ceaps e contratos publicos`

- [x] 008. Implementar migração para as tabelas de grafo relacional: `nos_rede` e `conexoes_rede` com índices bidirecionais (`idx_conexoes_origem`, `idx_conexoes_destino`, `idx_conexoes_busca`)[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): adiciona tabelas e indices para modelagem de grafos`

- [x] 009. Implementar migração para as tabelas `cache_consultas_diario`, `registros_profissionais` (OAB/CFM/CREA) e `alertas_incompatibilidade`[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): adiciona tabelas de diários oficiais e conselhos de classe`

- [x] 010. Implementar tabelas virtuais FTS5 (`politicos_fts` e `fornecedores_fts`) e triggers de sincronização automática[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): configura tabelas virtuais fts5 para busca textual`

- [x] 011. Implementar métodos de inserção em lote (`batch_insert_receitas`, `batch_insert_despesas`) utilizando transações explícitas `BEGIN`/`COMMIT`[cite: 1].  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): implementa rotinas de batch insert com transacao`

---

## Fase 3: Motor de Auditoria & Heurísticas (`crates/auditor`)

- [x] 012. Criar `crates/auditor/Cargo.toml` com dependências para `crates/storage`, `chrono`, `serde`, `thiserror` e `anyhow`.  
  *Validação:* `cargo check -p auditor`  
  *Commit:* `feat(auditor): configura crate de auditoria e analise deterministica`

- [x] 013. Implementar módulo `crates/auditor/src/combustivel.rs` com a heurística de capacidade volumétrica de veículos leves (> 80 litros) e cálculo de litros via preço ANP[cite: 1].  
  *Validação:* `cargo test -p auditor -- combustivel`  
  *Commit:* `feat(auditor): implementa heuristica de anomalia de combustivel do fiat uno`

- [x] 014. Implementar módulo `crates/auditor/src/ubiquidade.rs` para detecção de despesas presenciais em cidades distantes com velocidade teórica > 800 km/h[cite: 1].  
  *Validação:* `cargo test -p auditor -- ubiquidade`  
  *Commit:* `feat(auditor): implementa detector de inconsistencia geotemporal`

- [x] 015. Implementar módulo `crates/auditor/src/conflito_oab.rs` para verificação de incompatibilidade de secretários municipais ativos com OAB em situação "Regular" (Art. 28 da Lei 8.906/94)[cite: 1].  
  *Validação:* `cargo test -p auditor -- conflito_oab`  
  *Commit:* `feat(auditor): implementa regra do art 28 da oab para secretarios`

- [x] 016. Implementar módulo `crates/auditor/src/triangulacao.rs` para identificar contratos diretos (PNCP) com empresas de doadores PF em até 180 dias após a posse[cite: 1].  
  *Validação:* `cargo test -p auditor -- triangulacao`  
  *Commit:* `feat(auditor): implementa detector de triangulacao doacao x contrato direto`

- [x] 017. Implementar `crates/auditor/src/fornecedores_fantasmas.rs` para sinalizar fornecedores recém-criados (< 30 dias) ou com situação cadastral inapta na Receita[cite: 1].  
  *Validação:* `cargo test -p auditor -- fantasmas`  
  *Commit:* `feat(auditor): implementa verificador de fornecedores inaptos e recentes`

---

## Fase 4: Ingestão em Streaming & Normalização (`crates/ingestion`)

- [x] 018. Criar `crates/ingestion/Cargo.toml` com `csv-async`, `async-zip`, `reqwest`, `tokio`, `crates/storage`, `encoding_rs` e `serde`[cite: 1].  
  *Validação:* `cargo check -p ingestion`  
  *Commit:* `feat(ingestion): configura crate de ingestao assincrona`

- [x] 019. Implementar `crates/ingestion/src/normalizer.rs` com funções para mascaramento de CPF, limpeza de raiz de CNPJ e conversão de encoding ISO-8859-1 para UTF-8[cite: 1].  
  *Validação:* `cargo test -p ingestion -- normalizer`  
  *Commit:* `feat(ingestion): implementa normalizacao de documentos e encodings`

- [x] 020. Implementar `crates/ingestion/src/tse_streaming.rs` para descompactar e processar streams de CSVs do TSE (`consulta_cand` e `receitas_candidatos`) sem carregar arquivos na RAM[cite: 1].  
  *Validação:* `cargo test -p ingestion -- tse`  
  *Commit:* `feat(ingestion): implementa parser streaming de receitas do tse`

- [x] 021. Implementar parser para os arquivos de prestação de contas de despesas eleitorais (`despesas_candidatos`) do TSE[cite: 1].  
  *Validação:* `cargo test -p ingestion -- despesas_tse`  
  *Commit:* `feat(ingestion): implementa parser de despesas eleitorais`

- [x] 022. Implementar consumidor da API de Dados Abertos da Câmara dos Deputados para extração dos registros anuais da CEAP[cite: 1].  
  *Validação:* `cargo test -p ingestion -- ceap`  
  *Commit:* `feat(ingestion): adiciona integracao com dados abertos da camara para ceap`

- [x] 023. Implementar cliente assíncrono para a API aberta do PNCP (Portal Nacional de Contratações Públicas) para busca de contratos por CNPJ[cite: 1].  
  *Validação:* `cargo test -p ingestion -- pncp`  
  *Commit:* `feat(ingestion): implementa extrator de contratos do pncp`

- [x] 024. Implementar cliente sob demanda para o Querido Diário com busca restrita por doador, termos de nomeação e município[cite: 1].  
  *Validação:* `cargo test -p ingestion -- querido_diario`  
  *Commit:* `feat(ingestion): implementa consulta on demand do querido diario`

- [x] 025. Implementar scraper sob demanda para consulta de status no Cadastro Nacional dos Advogados (CNA/OAB) salvando payload em cache[cite: 1].  
  *Validação:* `cargo test -p ingestion -- oab`  
  *Commit:* `feat(ingestion): implementa consulta sob demanda ao cna da oab`

---

## Fase 5: Motor de Grafos e Métricas de Rede (`crates/graph`)

- [x] 026. Criar `crates/graph/Cargo.toml` adicionando `petgraph`, `crates/storage`, `serde` e `thiserror`[cite: 1].  
  *Validação:* `cargo check -p graph`  
  *Commit:* `feat(graph): configura crate de modelagem e travessia de redes`

- [x] 027. Implementar `crates/graph/src/builder.rs` para sincronizar os registros das tabelas `nos_rede` e `conexoes_rede` para um grafo direcionado em memória (`petgraph::Graph`)[cite: 1].  
  *Validação:* `cargo test -p graph -- builder`  
  *Commit:* `feat(graph): implementa construcao de grafo direcionado a partir do sqlite`

- [x] 028. Implementar algoritmo de travessia e detecção de ciclos para rastrear conexões indiretas entre políticos e empresas em até 3 graus[cite: 1].  
  *Validação:* `cargo test -p graph -- travessia`  
  *Commit:* `feat(graph): adiciona algoritmo de busca de caminhos em ate 3 graus`

- [x] 029. Implementar algoritmo de detecção de Fornecedor "Hub" calculando centralidade de intermediação (*Betweenness Centrality*) e concentração por coligação[cite: 1].  
  *Validação:* `cargo test -p graph -- hub`  
  *Commit:* `feat(graph): implementa algoritmo de deteccao de fornecedores hub`

---

## Fase 6: Servidor de Aplicação HTTP (`crates/server`)

- [x] 030. Criar `crates/server/Cargo.toml` com `axum`, `tower-http` (CORS e compressão), `tokio`, `serde`, `serde_json` e as crates internas do workspace[cite: 1].  
  *Validação:* `cargo check -p server`  
  *Commit:* `feat(server): define dependencias do servidor http axum`

- [x] 031. Implementar rotas de busca textual FTS5 (`/api/v1/busca?q=...`) retornando políticos, doadores e fornecedores em formato unificado[cite: 1].  
  *Validação:* `cargo test -p server -- busca`  
  *Commit:* `feat(server): adiciona endpoint de busca unificada via fts5`

- [x] 032. Implementar rota de dossiê do político (`/api/v1/politico/{id}`), incluindo histórico de bens, doadores e foto em base64/blob[cite: 1].  
  *Validação:* `cargo test -p server -- politico`  
  *Commit:* `feat(server): adiciona rota de dossie consolidado do candidato`

- [x] 033. Implementar rota do grafo de relacionamentos (`/api/v1/grafo/{id}?grau=2`) retornando nós e arestas formatados para Cytoscape/ECharts[cite: 1].  
  *Validação:* `cargo test -p server -- grafo`  
  *Commit:* `feat(server): implementa endpoint de extracao de subgrafos`

- [x] 034. Implementar rotas do motor de auditoria (`/api/v1/auditoria/alertas`), listando anomalias ordenadas por severidade e filtros por município/ano[cite: 1].  
  *Validação:* `cargo test -p server -- alertas`  
  *Commit:* `feat(server): adiciona endpoint de listagem de anomalias detectadas`

- [x] 035. Implementar rota sob demanda para investigação de nomeações e conselhos de classe (`/api/v1/investigar/nomeacao/{doador_id}`)[cite: 1].  
  *Validação:* `cargo test -p server -- investigar`  
  *Commit:* `feat(server): adiciona rota de disparos sob demanda de diarios e oab`

- [x] 036. Montar o binário executável (`crates/server/src/main.rs`) que aplica migrações no arranque, instancia o pool de conexões e sobe a API com graceful shutdown[cite: 1].  
  *Validação:* `cargo run -p server -- --version`  
  *Commit:* `feat(server): consolida ponto de entrada principal e inicializacao da api`

---

## Fase 7: Frontend Web & Visualização (`web/`)

- [x] 037. Inicializar projeto SvelteKit/Tailwind em `web/` com estrutura de componentes para painel, dossiê e busca.  
  *Validação:* `cd web && npm run check`  
  *Commit:* `feat(web): inicializa esqueleto do frontend sveltekit com tailwind`

- [x] 038. Criar componente de dossiê do político exibindo foto oficial do TSE, cartões de métricas, histórico de patrimônio e tabela de doadores[cite: 1].  
  *Validação:* `cd web && npm run build`  
  *Commit:* `feat(web): cria componente visual de dossie do politico`

- [x] 039. Integrar visualizador interativo de grafos (Cytoscape.js) colorindo nós por categoria e destacando arestas com anomalia[cite: 1].  
  *Validação:* `cd web && npm run build`  
  *Commit:* `feat(web): implementa componente interativo de grafos com cytoscape`

- [x] 040. Criar painel de alertas do auditor exibindo discrepâncias de combustível, incompatibilidade de OAB e contratos pós-eleição[cite: 1].  
  *Validação:* `cd web && npm run build`  
  *Commit:* `feat(web): adiciona tela de ranking de anomalias com filtros`

---

## Fase 8: Teste de Integração Ponta a Ponta

- [x] 041. Criar teste de integração completo em `tests/e2e_pipeline.rs`: popula dados simulados de um candidato e um doador advogado nomeado em secretaria, roda o auditor e valida o alerta[cite: 1].  
  *Validação:* `cargo test --test e2e_pipeline`  
  *Commit:* `test(e2e): valida pipeline completo de ingestao auditoria e grafo`

- [x] 042. Validar build multi-stage final via Docker Compose e verificar subida completa de backend e banco persistido em `./data`[cite: 1].  
  *Validação:* `docker compose up --build -d && docker compose ps`  
  *Commit:* `chore(release): valida empacotamento completo em containers docker`


---

## Fase 9: Configurações, Ingestão Atemporal e Exportação

- [x] 043. Criar migração em `crates/storage` implementando a tabela `historico_sincronizacao` e métodos de consulta de histórico.  
  *Validação:* `cargo test -p storage`  
  *Commit:* `feat(storage): adiciona tabela e consultas para historico de sincronizacoes`

- [x] 044. Implementar rotas e métodos de sondagem HTTP HEAD para verificar disponibilidade de dados do TSE.  
  *Validação:* `cargo test -p server`  
  *Commit:* `feat(ingestion): adiciona sondagem dinamica http head para bases do tse`

- [x] 045. Atualizar rotas e parsers para aceitar qualquer ano via parâmetro numérico, emitindo eventos de progresso e inserindo em lotes.  
  *Validação:* `cargo test -p server`  
  *Commit:* `feat(ingestion): parametrizacao atemporal e canal de progresso no parser do tse`

- [x] 046. Criar módulo `crates/server/src/routes/config.rs` implementando endpoints `GET /api/v1/config/status` (tamanho do sqlite e registros por tabela) e `GET /api/v1/config/jobs/{job_id}` (monitoramento de progresso).  
  *Validação:* `cargo test -p server -- config_status`  
  *Commit:* `feat(server): adiciona rotas de status do banco e rastreamento de jobs`

- [x] 047. Implementar rotas `GET /api/v1/config/tse/verificar/{ano}` e `POST /api/v1/config/tse/sincronizar` disparando o processamento em background via `tokio::spawn`.  
  *Validação:* `cargo test -p server -- config_sincronizar`  
  *Commit:* `feat(server): implementa endpoints de verificacao e disparo assincrono do tse`

- [x] 048. Implementar endpoints de exportação em `crates/server/src/routes/config.rs`: `GET /api/v1/config/exportar/banco` (stream do snapshot do SQLite) e `GET /api/v1/config/exportar/tabela/{nome_tabela}` (em CSV/JSON).  
  *Validação:* `cargo test -p server -- config_exportar`  
  *Commit:* `feat(server): adiciona endpoints de exportacao do banco e tabelas em stream`

- [x] 049. Criar rota `web/src/routes/configuracoes/+page.svelte` estruturando os cards de diagnóstico: tamanho do banco, contagem de registros e lista de anos já sincronizados.  
  *Validação:* `cd web && npm run check`  
  *Commit:* `feat(web): cria tela de configuracoes com dashboard de diagnostico do banco`

- [x] 050. Implementar seletor dinâmico de ano eleitoral (com checagem online via botão "Verificar Disponibilidade") e barra de progresso em tempo real consumindo o polling de jobs no SvelteKit.  
  *Validação:* `cd web && npm run build`  
  *Commit:* `feat(web): adiciona seletor atemporal e monitoramento visual de ingestao`

- [x] 051. Adicionar na interface de configurações os botões de exportação de dados (`.sqlite` e `.csv`) e dropzone de contingência para upload manual de arquivos locais.  
  *Validação:* `cd web && npm run build`  
  *Commit:* `feat(web): implementa acoes de exportacao de dados e upload manual`

- [x] 052. Criar teste de integração em `tests/config_pipeline.rs` validando a verificação de ano, disparo de job em background e geração de exportação de snapshot.  
  *Validação:* `cargo test --test config_pipeline`  
  *Commit:* `test(e2e): valida pipeline completo de configuracao sincronizacao e exportacao`
## Fase 10: Módulo de Benefícios Sociais e Auxílio Emergencial

- [x] 053. Criar migração em `crates/storage` com tabelas `beneficios_emergenciais` e `alertas_beneficio_indevido`, incluindo índices de busca rápida.  
  *Validação:* `cargo test -p storage -- beneficio`  
  *Commit:* `feat(storage): adiciona tabelas para auditoria de auxilio emergencial`

- [x] 054. Implementar parser de streaming em `crates/ingestion` para arquivos de pagamentos de benefícios da CGU/Brasil.IO.  
  *Validação:* `cargo test -p ingestion -- auxilio`  
  *Commit:* `feat(ingestion): implementa parser streaming de auxilio emergencial`

- [x] 055. Implementar regras em `crates/auditor` flagrando recebimento com mandato vigente ou bens declarados superiores a R$ 300.000,00.  
  *Validação:* `cargo test -p auditor -- auxilio_indevido`  
  *Commit:* `feat(auditor): implementa heuristica de recebimento indevido de auxilio`

- [x] 056. Adicionar rota `/api/v1/auditoria/auxilio-indevido` no Axum e exibir badge/cartão de benefício indevido no dossiê do político em SvelteKit.  
  *Validação:* `cargo check -p server && cd web && npm run build`  
  *Commit:* `feat(web): integra alertas de auxilio emergencial no dossie do politico`

## Fase 11: Ingestão Dual da Câmara dos Deputados (CEAP)

- [x] 057. Implementar parser de streaming para os arquivos estáticos anuais compactados da CEAP (`Ano-{ano}.csv.zip`) em `crates/ingestion/src/camara/bulk.rs`.  
  *Validação:* `cargo test -p ingestion -- camara_bulk`  
  *Commit:* `feat(ingestion): implementa extrator em streaming de dumps anuais da ceap`

- [x] 058. Implementar cliente assíncrono com suporte a paginação HATEOAS (`links.next`) e rate-limit para a API REST v2 da Câmara em `crates/ingestion/src/camara/api.rs`.  
  *Validação:* `cargo test -p ingestion -- camara_api`  
  *Commit:* `feat(ingestion): adiciona consumo paginado hateoas da api da camara`

- [x] 059. Implementar rota `POST /api/v1/config/camara/sincronizar` no Axum suportando alternância entre modo estático e modo API REST via background task.  
  *Validação:* `cargo test -p server -- camara_sincronizar`  
  *Commit:* `feat(server): adiciona endpoint de sincronizacao configuravel da ceap`

- [ ] 060. Adicionar card de sincronização da Câmara no SvelteKit com alternância entre carga anual em massa e API REST paginada.  
  *Validação:* `cd web && npm run build`  
  *Commit:* `feat(web): cria controles de sincronizacao estatica e api da camara`

- [ ] 061. Criar teste de integração ponta a ponta simulando carga de lote de notas fiscais da Câmara e verificação na tabela `despesas_parlamentares`.  
  *Validação:* `cargo test --test camara_pipeline`  
  *Commit:* `test(e2e): valida ingestao dual da camara com persistencia no sqlite`
