# Sincronização Unificada e Idempotência de Dados no Radar Cívico

Este documento detalha o funcionamento, tempo de execução, arquitetura de persistência e proteção contra duplicação de dados implementados no recurso **"Sincronizar Tudo"** do Radar Cívico.

---

## 1. Visão Geral do Recurso "Sincronizar Tudo"

O botão **"Sincronizar Tudo"** foi integrado na interface de **Configurações e Gestão de Dados** (`web/src/routes/configuracoes/+page.svelte`) e exposto via API REST nos endpoints:
* `POST /api/v1/config/sincronizar-tudo`
* `POST /api/config/sincronizar-tudo`

Ele orquestra um pipeline sequencial e não bloqueante (`tokio::spawn`) que varre as fontes governamentais primárias de forma integrada, aplicando controle estrito de idempotência e acionando o Motor de Auditoria Analítico ao final do processo.

---

## 2. Tempo de Execução Estimado

O tempo total de sincronização varia conforme o escopo e os anos selecionados:

| Etapa do Pipeline | Fonte Primária | Volume Médio por Ano | Tempo Estimado |
| :--- | :--- | :--- | :--- |
| **1. Integridade & Pragmas** | SQLite Local | Aplicação de PRAGMAs WAL, cache e índices | **< 1 segundo** |
| **2. Câmara dos Deputados (CEAP)** | Portal da Câmara (`Ano-{ano}.csv.zip`) | Centenas de milhares de notas fiscais | **1 a 3 minutos** |
| **3. Eleições TSE** | Repositório CKAN Dados Abertos TSE | Pacotes ZIP oficiais consolidados | **5 a 12 minutos** |
| **4. Cadastros & Vínculos** | Receita Federal (QSA) e CNA / OAB | Base de sócios e registros profissionais | **< 5 segundos** |
| **5. Motor de Auditoria** | Heurísticas determinísticas | Varredura analítica completa sobre o banco | **10 a 40 segundos** |

### Resumo:
* **Execução Padrão de Manutenção (Ano Corrente/Eleitoral Recente):** entre **8 e 15 minutos**.
* Toda a operação ocorre em **segundo plano**, com telemetria exibida em tempo real no monitor de jobs da tela de configurações (`activeJob`).

### 2.1 Concorrência do SQLite durante a sincronização

O backend reserva cada job em uma transação imediata antes de iniciar o worker. Sincronizações da mesma família e ano não podem iniciar simultaneamente, mesmo quando solicitadas por abas diferentes. CEAP BULK e API compartilham a mesma família; TSE e TSE CKAN também. Importadores sem ano declarado reservam toda a família. **Sincronizar Tudo** e deduplicação em massa conflitam com qualquer job de configuração ou importador ativo. A API responde `409 Conflict` e informa `job_id` para tarefas de configuração ou `importer_id` para importadores. A interface acompanha o job de configuração informado.

As consultas e gravações das rotinas de sincronização são executadas em tarefas bloqueantes, fora das threads HTTP. Na CEAP, a conexão é emprestada apenas para cada lote e devolvida antes da atualização de progresso. Os snapshots usam `storage::transaction_immediate` com retentativas para `SQLITE_BUSY`/`SQLITE_LOCKED`; leituras de snapshots também toleram contenção transitória do banco em memória compartilhada.

A consulta automática de progresso agenda a próxima chamada depois da resposta anterior. Em caso de falha, o intervalo aumenta até 15 segundos; ao recuperar, retorna ao intervalo normal. A troca de job e a saída da página cancelam as consultas pendentes. As consultas de importadores seguem o mesmo mecanismo.

Uma resposta após 15 segundos, isoladamente, **não identifica a causa**: tanto o pool quanto o `busy_timeout` utilizam esse prazo. Os logs agora distinguem `stage=pool`, `stage=transaction`, `stage=operation` e `stage=completion`, registrando tempos de espera e códigos SQLite nas falhas. Atrasos relevantes aparecem no nível `info`; detalhes das aquisições aparecem em `debug`.

### 2.2 Streaming e resultados da CEAP

O download é gravado em blocos em arquivo temporário, limitado a **1 GiB**. O CSV é descompactado para outro arquivo temporário, limitado a **4 GiB**, e lido com `csv-async` em lotes de **500 registros**. Cada registro tem limite de **1 MiB**, incluindo campos entre aspas com múltiplas linhas. O runtime HTTP não faz a descompactação nem as gravações. Os arquivos temporários são removidos ao terminar a tarefa; uma carga pode ocupar até aproximadamente **5 GiB de disco temporário**.

O importador CEAP usa o mesmo processamento da sincronização individual e unificada. Duplicatas continuam sendo ignoradas. O modo API exige IDs de deputados e informa falhas de consulta ou gravação por deputado.

- `CONCLUIDO`: todos os pacotes ou deputados solicitados foram processados, mesmo quando não havia novas despesas por causa da deduplicação.
- `PARCIAL`: parte da carga foi confirmada e outra parte falhou. Lotes já confirmados permanecem na base.
- `ERRO`: falha no download, no schema ou no processamento sem uma etapa bem-sucedida. A existência de dados antigos não transforma a falha em sucesso.

As rotinas legadas que apenas consultam TSE, CEAP, QSA ou PNCP na base local informam explicitamente que não baixaram novos dados. Falhas remotas do Querido Diário ficam registradas como erro no cache e não contam como consultas bem-sucedidas.

### 2.3 Persistência da conclusão e recuperação

O resultado final é colocado em uma fila de conclusões antes de disputar a trava SQLite. Em bancos em arquivo, a fila usa arquivos JSON gravados atomicamente ao lado da base; para `data/radar_civico.db`, o diretório é `data/radar_civico.job-completions/`. Esse diretório deve acompanhar o volume de dados. Bancos em memória usam uma fila em memória.

O endpoint de status pode consultar essa fila sem adquirir uma conexão do pool. O campo `persistencia_pendente` indica que o resultado está disponível e aguarda gravação no SQLite; a interface continua consultando até a confirmação. Bloqueios e timeouts de pool recebem novas tentativas com intervalo crescente, até cinco segundos entre tentativas.

O snapshot final e o histórico são confirmados na mesma transação, liberando a reserva do job. A reaplicação é idempotente e não duplica o histórico. No arranque, conclusões pendentes são reaplicadas antes de os workers antigos serem marcados como interrompidos. Erros permanentes de banco ou de disco são registrados para diagnóstico; a fila preserva o resultado quando sua gravação em disco foi possível.

### 2.4 Validação de concorrência

As regressões usam SQLite em arquivo e servidor HTTP local mockado para verificar admissão simultânea, conflito entre endpoints, leituras durante escrita em WAL, conclusão com pool esgotado, recuperação após reinício, idempotência do histórico, limites dos lotes e falhas após confirmação parcial. O polling é testado com timers controlados, incluindo cancelamento e recuperação após erros.

```bash
cargo check --workspace --locked
cargo test --all --locked
npm run test:polling --prefix web
npm run check --prefix web
npm run build --prefix web
```

---

## 3. Idempotência e Comportamento do Banco de Dados

### Pergunta Fundamental:
> *"O sistema pega o que já foi salvo no banco ou joga tudo de novo por cima?"*

### Resposta e Garantias Técnicas:
O sistema **reaproveita os registros existentes, atualiza dados modificados e rejeita duplicatas**. Nenhuma nota fiscal, político ou alerta é duplicado ao rodar a sincronização repetidas vezes.

### Detalhamento por Tabela:

#### 1. Políticos e Candidaturas (`politicos` e `candidaturas`)
* **Mecanismo:** `ON CONFLICT(sq_candidato) DO UPDATE SET` e `ON CONFLICT(politico_id, ano_eleicao, cargo) DO UPDATE SET`.
* **Comportamento:** Se o político ou a candidatura já existirem, o SQLite atualiza os campos com os dados novos sem criar novas linhas.

#### 2. Despesas Parlamentares da CEAP (`despesas_parlamentares`)
* **Migração 14 (`deduplicar_e_criar_indice_unico_despesas_parlamentares`):**
  * Removeu duplicatas históricas preservando o menor ID.
  * Criou o índice único idempotente:
    ```sql
    CREATE UNIQUE INDEX IF NOT EXISTS idx_desp_parl_dedup ON despesas_parlamentares(
        casa_legislativa, parlamentar_nome, data_emissao, fornecedor_cnpj_cpf, valor_liquido, COALESCE(numero_documento, '')
    );
    ```
* **Inserção em Lote:** `crates/storage/src/batch.rs` consome `INSERT OR IGNORE INTO despesas_parlamentares`.
* **Comportamento:** Ao rodar novamente a carga da Câmara do mesmo ano, todas as despesas já existentes são **automaticamente ignoradas** sem erro e sem inflar os gastos parlamentares.

#### 3. Quadro Societário (QSA) e Registros Profissionais (OAB)
* **Tabela `empresas_qsa`:** Inserção via `INSERT OR REPLACE INTO empresas_qsa (...)`.
* **Tabela `registros_profissionais`:** Restrição única `UNIQUE(orgao_emissor, seccional_uf, numero_registro)` com `INSERT OR REPLACE`.
* **Comportamento:** Registros atualizados substituem com segurança os anteriores.

#### 4. Motor de Auditoria (`alertas_auditoria`)
* **Mecanismo:** Checagem prévia de existência por chave referencial (`SELECT 1 FROM alertas_auditoria WHERE tipo = ? AND detalhes_json LIKE ? LIMIT 1`).
* **Comportamento:** Não gera alertas repetidos para uma mesma despesa ou político já auditado.

---

## 4. Estrutura da Requisição e Resposta da API

### `POST /api/v1/config/sincronizar-tudo`

#### Payload (opcional):
```json
{
  "ano_eleitoral": 2024,
  "ano_fiscal": 2024,
  "incluir_tse": true,
  "incluir_camara": true,
  "incluir_receita": true,
  "incluir_auditoria": true
}
```

#### Resposta de Sucesso (`202 ACCEPTED`):
```json
{
  "job_id": "ddb81aa9-f5f3-4de0-bbd2-2e31d10987fe",
  "status": "PROCESSANDO",
  "mensagem": "Processo de sincronização unificada iniciado com sucesso em segundo plano."
}
```

---

## 5. Interface Gráfica (Frontend)

Na página de **Configurações** (`/configuracoes`):
1. **Botão de Destaque no Cabeçalho:** `🔄 Sincronizar Tudo`, localizado ao lado de "Atualizar Métricas".
2. **Card Banner na Seção de Ingestão:** Exibe resumo do pipeline orquestrado e botão de disparo.
3. **Modal de Confirmação & Escopo:** Permite escolher quais fontes serão sincronizadas, exibe o aviso claro sobre idempotência e estimativa de tempo de execução.
4. **Monitor em Tempo Real:** Barra de progresso (0% a 100%), status do job (`PROCESSANDO`, `CONCLUIDO`, `ERRO`) e logs em streaming com timestamps UTC.
5. **Histórico de Sincronizações:** O evento `SINCRONIZAR_TUDO` é automaticamente registrado no card de metadados do cabeçalho e na tabela `historico_sincronizacao`.
