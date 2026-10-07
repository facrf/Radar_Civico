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
3. O `PRAGMA busy_timeout` tradicional do SQLite **não** suspende a execução durante bloqueios de tabela em shared cache, retornando o erro `262` imediatamente.

### Solução Arquitetural: `transaction_immediate`
Para eliminar essa falha, foi introduzida a função `storage::connection::transaction_immediate`:
* **Comportamento `IMMEDIATE`:** Adquire a trava de escrita no momento em que a transação é aberta (`BEGIN IMMEDIATE;`), prevenindo impasses de deadlock durante upgrades posteriores.
* **Retentativa com Backoff Exponencial:** Caso a trava de banco esteja temporariamente retida por outra transação ativa, o worker realiza retentativas em loop com intervalo incremental até 10 segundos antes de propagar qualquer erro.
* **Pragmas Obrigatórios Padronizados:**
  ```sql
  PRAGMA journal_mode = WAL;
  PRAGMA synchronous = NORMAL;
  PRAGMA cache_size = -64000;
  PRAGMA temp_store = MEMORY;
  PRAGMA foreign_keys = ON;
  PRAGMA busy_timeout = 15000;
  ```

Todas as operações em lote (`crates/storage/src/batch.rs`), conectores de ingestão (`crates/ingestion/src/importers/connectors.rs`), migrações (`crates/storage/src/migrations/mod.rs`) e rotas de upload (`crates/server/src/routes/config.rs`) foram atualizadas para consumir esse protocolo.

---

## 2. Deduplicação e Fusão de Políticos (`crates/server/src/politicos_duplicados.rs`)

A rotina de consolidação de registros repetidos (`mesclar_duplicados_transacional`) foi reforçada para manter total consistência referencial:
* **Migração de `despesas_campanha`:** Previamente apenas `receitas_campanha` e `bens_candidato` eram reatribuídos ao canônico, gerando risco de violação de chave estrangeira ao expurgar a candidatura duplicada. Agora, despesas também são migradas atomicamente.
* **Recálculo do Patrimônio:** O somatório de `total_bens_declarados` na tabela `politicos` é recalculado dinamicamente para refletir o acréscimo dos bens migrados.
* **Enriquecimento Cadastral:** Metadados como `cpf_mascarado`, `data_nascimento`, `ocupacao`, `grau_instrucao` e `sq_candidato` presentes no registro duplicado são preservados no registro canônico caso este último os possua em branco.

---

## 3. Heurística Determinística de Auxílio Emergencial (`crates/auditor/src/auxilio_indevido.rs`)

O cruzamento entre beneficiários do Auxílio Emergencial e a base eleitoral/mandatos passou por aprimoramentos para eliminação de falsos positivos:
* **Alinhamento de Máscara de CPF:** Tratamento robusto para registros de 6 dígitos (formato `***.123.456-**`) em relação aos dados completos do TSE.
* **Supressão de Falsos Positivos Homônimos:** Exigência de correspondência em tokens de sobrenome significativos, evitando alertas equivocados entre cidadãos homônimos que compartilham os mesmos 6 dígitos centrais mascarados.
* **Filtragem de Cargos Honoríficos:** Limpeza de prefixos como `VEREADOR`, `DEPUTADO`, `PROFESSOR` e `PASTOR` antes da comparação nominal.

---

## 4. Normalização Temporal na Camada Web (`web/`)

* Os componentes `MapaDespesas.svelte` e a visualização detalhada do político (`[id]/+page.svelte`) tiveram a rotina de formatação de datas ajustada para truncar o delimitador `T` de timestamps ISO-8601 antes de formatar no padrão brasileiro `DD/MM/AAAA`.
