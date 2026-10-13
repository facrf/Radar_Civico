# Auditoria, Concorrência e Integridade de Dados no Radar Cívico

Este documento descreve as melhorias arquiteturais, correções de concorrência e integridade implementadas no motor de dados e auditoria do Radar Cívico.

---

## 1. Concorrência e Gestão Transacional no SQLite (`crates/storage`)

### Diagnóstico de Contenção em Cache Compartilhado (`SQLITE_LOCKED_SHAREDCACHE`)
Em cenários de execução paralela massiva (como múltiplos workers de ingestão operando simultaneamente via `ImporterManager`), o SQLite operando com pool de conexões e memória compartilhada (`cache=shared`) pode disparar o erro:
```text
SqliteFailure(Error { code: DatabaseLocked, extended_code: 262 }, Some("database table is locked"))
```
Isso ocorre porque:
1. O SQLite em conexões com cache compartilhado utiliza travas a nível de tabela (*table-level locks*).
2. Transações iniciadas por padrão com `DEFERRED` começam em modo leitura e tentam upgrade para escrita no primeiro `INSERT`/`UPDATE`. Quando múltiplos threads realizam esse upgrade simultaneamente, o SQLite não consegue promover ambas e rejeita uma delas imediatamente.
3. O `PRAGMA busy_timeout` tradicional do SQLite **não** suspende a execução durante bloqueios de tabela em shared cache, retornando o erro `262` imediatamente caso o timeout interno da aplicação não seja alinhado com o tempo de retenção do driver.

### Solução Arquitetural: `transaction_immediate` e `transaction_immediate_ref`
Para eliminar contenções e erros de `database locked` sob operações concorrentes:
* **Comportamento `IMMEDIATE`:** Adquire a trava de escrita no momento exato em que a transação é aberta (`BEGIN IMMEDIATE;`), prevenindo deadlocks durante upgrades posteriores.
* **Alinhamento de Timeouts e Backoff Progressivo:** O timeout de conexões no pool, a rotina de transações imediatas e o SQLite foram unificados em **60 segundos** (`busy_timeout = 60000`). Se o banco estiver retido por outra escrita (códigos `DatabaseBusy`, `DatabaseLocked`, 262 e 5), o worker executa retentativas com backoff progressivo (25ms a 250ms com jitter) até o limite de 60s.
* **Pragmas Obrigatórios Padronizados:**
  ```sql
  PRAGMA journal_mode = WAL;
  PRAGMA synchronous = NORMAL;
  PRAGMA cache_size = -64000;
  PRAGMA temp_store = MEMORY;
  PRAGMA foreign_keys = ON;
  PRAGMA busy_timeout = 60000;
  ```

### Desacoplamento da Projeção de Grafo e Contenção no Gerenciador de Importadores
* **Projeção Sob Demanda (`refresh_if_needed`):** Em bases volumosas (>2,6 milhões de relacionamentos), o rebuild incondicional de grafo retinha travas exclusivas por mais de 2 minutos. Foi adotada a verificação baseada em flags/triggers de sujeira (`refresh_if_needed`), impedindo que finalizações de jobs bloqueiem o banco quando não há alterações no grafo.
* **Liberação de Lock em Memória (`ImporterManager::start`):** O lock de leitura/escrita em memória dos contextos de importação agora é liberado antes da chamada de gravação de estado no banco de dados (`save_exclusive`), evitando que a sondagem de status `/api/importers` bloqueie threads de controle.
* **Tratamento de Contenção HTTP 503:** Erros residuais de contenção temporária de escrita são convertidos pelo servidor em `503 Service Unavailable` com mensagem orientativa, em vez de falhas internas genéricas.

---

## 2. Deduplicação e Fusão de Políticos (`crates/server/src/politicos_duplicados.rs`)

A rotina de consolidação de registros repetidos (`mesclar_duplicados_transacional`) foi reforçada para manter total consistência referencial:
* **Migração de `despesas_campanha`:** Previamente apenas `receitas_campanha` e `bens_candidato` eram reatribuídos ao canônico, gerando risco de violação de chave estrangeira ao expurgar a candidatura duplicada. Agora, despesas também são migradas atomicamente.
* **Recálculo do Patrimônio:** O somatório de `total_bens_declarados` na tabela `politicos` é recalculado dinamicamente para refletir o acréscimo dos bens migrados.
* **Enriquecimento Cadastral:** Metadados como `cpf_mascarado`, `data_nascimento`, `ocupacao`, `grau_instrucao` e `sq_candidato` presentes no registro duplicado são preservados no registro canônico caso este último os possua em branco.

---

### 3. Heurística Determinística de Auxílio Emergencial (`crates/auditor/src/auxilio_indevido.rs`)

O cruzamento entre beneficiários do Auxílio Emergencial e a base eleitoral/mandatos passou por aprimoramentos para eliminação de falsos positivos:
* **Alinhamento de Máscara de CPF:** Tratamento robusto para registros de 6 dígitos (formato `***.123.456-**`) em relação aos dados completos do TSE.
* **Supressão Estrita de Homônimos de Dígitos:** Para registros com CPF mascarado (apenas os 6 dígitos centrais), o auditor descarta correspondências caso os cidadãos compartilhem unicamente um sobrenome de alta frequência no Brasil (como *Silva*, *Santos*, *Oliveira*, *Souza*, etc.) e possuam o primeiro nome divergente (ex.: *Carlos da Silva* vs *Antônio da Silva*).
* **Filtragem de Cargos Honoríficos:** Limpeza de prefixos como `VEREADOR`, `DEPUTADO`, `PREFEITO`, `PROFESSOR` e `PASTOR` antes da tokenização e comparação nominal.

---

## 4. Normalização Temporal e Interface Web Não-Bloqueante (`web/`)

* **Normalização de Timestamps:** Os componentes `MapaDespesas.svelte` e a visualização detalhada do político (`[id]/+page.svelte`) truncam o delimitador `T` de timestamps ISO-8601 antes de formatar no padrão brasileiro `DD/MM/AAAA`.
* **Substituição de `window.alert` por Toasts Reativos:** Todos os diálogos bloqueantes de navegador foram substituídos por um sistema visual de toasts animados com auto-dispensa (`ToastNotificacao`) na página de configurações (`/configuracoes`) e feedback inline na unificação de políticos (`/politicos`), evitando a interrupção do loop de polling e do ciclo de eventos do navegador.

---

## 5. Novas Heurísticas do Motor de Auditoria e Backups Quentes

### 5.1 Evolução Patrimonial Desproporcional (`EVOLUCAO_PATRIMONIAL`)
* **Regra:** Identifica candidatos cujo patrimônio declarado cresceu mais de 300% entre eleições consecutivas com acréscimo absoluto mínimo de R$ 200.000,00.
* **Gatilho de Severidade:** Classificado como `CRITICA` caso o salto supere 500% ou acréscimo absoluto supere R$ 1.000.000,00.
* **Consumo no Dossiê:** Exibido em aba dedicada (`/politicos/:id`) com timeline comparativa e barras proporcionais.

### 5.2 Doadores Incompatíveis com Programas Sociais (`DOADOR_INCOMPATIVEL`)
* **Regra:** Identifica cidadãos inscritos como beneficiários de programas sociais (Auxílio Emergencial / Bolsa Família) que realizaram doações eleitorais de R$ 1.000,00 ou mais para campanhas políticas.
* **Cruzamento:** Chave composta por `cpf_mascarado` e tokens de nome normalizados.

### 5.3 Conluio / Sócios Comuns em Contratações Públicas (`CONLUIO_LICITACAO`)
* **Regra:** Cruzamento entre `contratos_publicos` (PNCP) e `empresas_qsa` (Receita Federal) que identifica quando um mesmo órgão público contrata duas ou mais empresas distintas que compartilham o mesmo quadro de sócios.

### 5.4 Sobrepreço em Abastecimentos da CEAP (`COMBUSTIVEL_SOBREPRECO`)
* **Regra:** Compara o valor unitário faturado por litro nas notas fiscais de combustível com a tabela histórica de preços de combustíveis da ANP (referência padrão R$ 5,80/L). Notas com preço superior a 150% (> R$ 8,70/L) são sinalizadas.

### 5.5 Backup Online a Quente (Hot SQLite Backup) e Webhooks
* **Snapshot Online:** A API `rusqlite::backup::Backup` cria cópias físicas completas do banco de produção em segundo plano, sem travar threads de escrita ou leitura, armazenadas em `data/backups/`.
* **Notificações:** Suporte a webhooks externos (`POST /api/v1/config/webhook/test`) para alertas e integrações automatizadas.

---

## 6. Escalabilidade de Grafos e Betweenness Centrality (`crates/graph`)

* **Centralidade Brandes Amostrada:** O algoritmo exato de Brandes possui complexidade $O(|V| \cdot |E|)$, tornando o cálculo impraticável para a base com mais de 2,65 milhões de arestas. A função `calcular_betweenness_centrality_amostrado` introduziu amostragem uniforme para grafos com mais de 500 nós (máximo de 250 pivôs com reescalonamento proporcional), mantendo a precisão analítica e prevenindo congelamento de threads.
* **Eliminação de Pânicos em Grafos:** Removidos todos os `.unwrap()` não-seguros na travessia BFS e propagação de dependências em `hub.rs`.

---

## 7. Manutenção Contínua do WAL e Resiliência Transacional (`crates/storage`)

* **Checkpoint Passivo Periódico (`checkpoint_passivo`):** Adicionada a função `storage::checkpoint_passivo` invocando `PRAGMA wal_checkpoint(PASSIVE);` ao término do processamento de pacotes do TSE. Isso permite ao SQLite reciclar as páginas do log sem aguardar o encerramento do serviço nem bloquear leituras em andamento.
* **Recuperação de Travas Envenenadas:** O endpoint `/api/import/status` emprega `.unwrap_or_else(|e| e.into_inner())` para ler a estrutura de progresso mesmo se uma thread anterior tiver entrado em pânico durante a escrita, garantindo disponibilidade ininterrupta da API.

