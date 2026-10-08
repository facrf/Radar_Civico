# Estratégias de Otimização da Ingestão de Dados (TSE e Cargas Massivas)

Documento de planejamento técnico com estratégias identificadas para acelerar as rotinas de ingestão e sincronização do TSE e outros módulos de dados do Radar Cívico.

---

## 1. Contexto e Diagnóstico

Durante a execução da ingestão dos dados eleitorais de 2024 (catálogo de 65 pacotes do CKAN/TSE), foram identificados os seguintes gargalos operacionais:
* **Download desnecessário de binários pesados:** O catálogo do CKAN inclui dezenas de arquivos de fotos (`foto_cand2024_*.zip`, até 2 GB cada) e propostas de governo (`proposta_governo_*.zip`), que somam mais de 15 GB mas não contêm nenhum arquivo CSV.
* **Classificação indevida de schemas:** Arquivos de redes sociais (`rede_social_candidato_2024_BRASIL.csv`, com 413.713 linhas) não eram filtrados e caíam no fallback de candidatos, gerando inserções e atualizações massivas sem necessidade.
* **Sobrecarga de queries individuais por registro:** A inserção em lote de candidatos executa 3 chamadas SQL por linha dentro do loop (`INSERT politicos`, `SELECT id`, `INSERT candidaturas`), totalizando 75.000 operações SQLite por lote de 25k registros em uma base de ~60 GB.
* **Bloqueio sequencial de I/O de rede e disco:** O fluxo é síncrono (`download -> parse -> insert`), deixando a CPU/disco ociosos durante o download e a rede ociosa durante a gravação.

---

## 2. Estratégias de Otimização

### Estratégia 1: Filtragem Prévia de URLs no Catálogo CKAN
* **Objetivo:** Evitar o download de pacotes que não contêm dados tabulares analíticos.
* **Escopo:** `crates/ingestion/src/tse_ckan.rs`
* **Ações:**
  * No método `descobrir_urls_tse_com_base`, filtrar as URLs retornadas pela API do CKAN antes de incluí-las na lista de processamento.
  * Ignorar recursos cujos nomes ou URLs contenham:
    * `foto_cand` / `fotos`
    * `proposta_governo`
    * `extrato_bancario`
    * `fefc_`
* **Impacto:** Elimina o tráfego de mais de 15 GB e economiza entre 30 a 60 minutos de rede.
* **Complexidade:** Muito Baixa.

---

### Estratégia 2: Validação Estrita de Schemas dos CSVs
* **Objetivo:** Evitar o processamento e parse de CSVs não suportados ou sem valor relacional direto.
* **Escopo:** `crates/ingestion/src/tse_ckan.rs`
* **Ações:**
  * No método `processar_csv_tse_str_com_progresso`, substituir o `else` genérico por validação explícita de cabeçalho.
  * Exigir que arquivos de candidatos contenham colunas obrigatórias como `NM_CANDIDATO` e `NR_CPF_CANDIDATO`.
  * Ignorar de imediato arquivos de metadados como `rede_social_candidato` se não houver tabela dedicada no schema.
* **Impacto:** Elimina o processamento inútil de mais de 400.000 registros, economizando cerca de 5 horas de processamento.
* **Complexidade:** Muito Baixa.

---

### Estratégia 3: Bulk Insert via Tabela Staging Temporária no SQLite (*Prioridade Máxima*)
* **Objetivo:** Reduzir o volume de operações SQL de 75.000 por lote para apenas 2 instruções em bloco.
* **Escopo:** `crates/storage/src/batch.rs`
* **Ações:**
  * Criar tabela temporária em memória na conexão ativa:
    ```sql
    CREATE TEMP TABLE IF NOT EXISTS staging_candidatos (
        sq_candidato TEXT,
        cpf_mascarado TEXT,
        nome_completo TEXT,
        nome_urna TEXT,
        data_nascimento TEXT,
        grau_instrucao TEXT,
        ocupacao TEXT,
        ano_eleicao INTEGER,
        cargo TEXT,
        numero_urna INTEGER,
        sigla_partido TEXT,
        uf TEXT,
        municipio TEXT,
        situacao_totalizacao TEXT
    );
    ```
  * Inserir o lote de 25k em massa na staging.
  * Executar a carga no banco principal via consultas relacionais em lote com `ON CONFLICT`:
    ```sql
    INSERT INTO politicos (sq_candidato, cpf_mascarado, nome_completo, nome_urna, data_nascimento, grau_instrucao, ocupacao)
    SELECT sq_candidato, cpf_mascarado, nome_completo, nome_urna, data_nascimento, grau_instrucao, ocupacao
    FROM staging_candidatos
    ON CONFLICT(sq_candidato) DO UPDATE SET
        cpf_mascarado = excluded.cpf_mascarado,
        nome_completo = excluded.nome_completo,
        nome_urna = excluded.nome_urna;

    INSERT INTO candidaturas (politico_id, ano_eleicao, cargo, numero_urna, sigla_partido, uf, municipio, situacao_totalizacao)
    SELECT p.id, s.ano_eleicao, s.cargo, s.numero_urna, s.sigla_partido, s.uf, s.municipio, s.situacao_totalizacao
    FROM staging_candidatos s
    JOIN politicos p ON p.sq_candidato = s.sq_candidato
    ON CONFLICT(politico_id, ano_eleicao, cargo) DO UPDATE SET
        numero_urna = excluded.numero_urna,
        sigla_partido = excluded.sigla_partido,
        uf = excluded.uf,
        municipio = excluded.municipio,
        situacao_totalizacao = excluded.situacao_totalizacao;
    ```
  * Limpar a tabela de staging ao final do lote (`DELETE FROM staging_candidatos`).
* **Impacto:** Aceleração estimada de 30x a 60x na gravação (tempo por lote de 25k cai de ~22 minutos para ~20 segundos).
* **Complexidade:** Média.

---

### Estratégia 4: Otimização Agressiva dos PRAGMAs do SQLite no Modo Ingestão
* **Objetivo:** Maximizar vazão de I/O e aproveitar a memória RAM disponível da máquina host.
* **Escopo:** `crates/storage/src/connection.rs`
* **Ações:**
  * Na função `aplicar_pragmas_ingestao`, definir:
    * `PRAGMA synchronous = OFF;` (durante a execução da ingestão em lote).
    * `PRAGMA cache_size = -500000;` (aloca ~500 MB de page cache na RAM para reter nós de B-Tree quentes).
    * `PRAGMA temp_store = MEMORY;` (garante que estruturas de ordenação e tabelas temporárias fiquem em RAM).
    * `PRAGMA wal_autocheckpoint = 100000;` (evita pausas frequentes para checkpoint no WAL durante inserções contínuas).
  * Restaurar `synchronous = NORMAL` ao finalizar a ingestão.
* **Impacto:** Reduz a contenção de disco e duplica a velocidade de escrita contínua.
* **Complexidade:** Baixa.

---

### Estratégia 5: Pipeline Produtor-Consumidor (Download Concorrente ao Processamento)
* **Objetivo:** Paralelizar o download de arquivos via rede com a gravação no banco de dados.
* **Escopo:** `crates/server/src/routes/config.rs`
* **Ações:**
  * Utilizar um canal `tokio::sync::mpsc::channel(2)` entre a tarefa de rede e a tarefa de banco.
  * Tarefa Produtora: Baixa o próximo pacote ZIP via HTTP enquanto o pacote anterior ainda está sendo descompactado e inserido.
  * Tarefa Consumidora: Recebe o buffer em memória e executa a ingestão no SQLite via thread de blocking.
* **Impacto:** Elimina o tempo de espera de download entre pacotes consecutivos.
* **Complexidade:** Média.

---

## 3. Matriz de Priorização e Plano de Execução

| Ordem | Estratégia | Esforço | Ganho Estimado |
| :---: | :--- | :---: | :--- |
| **1º** | Estratégia 1: Filtrar URLs no CKAN (ignorar fotos e propostas) | Muito Baixo | Economiza 15 GB de tráfego e ~45 min |
| **2º** | Estratégia 2: Validação estrita de cabeçalhos de CSV | Muito Baixo | Economiza ~5h evitando arquivo de redes |
| **3º** | Estratégia 3: Inserção em staging bulk SQL | Médio | Acelera inserção de 20 reg/s para > 1.000 reg/s |
| **4º** | Estratégia 4: Ajuste agressivo de PRAGMAs na ingestão | Baixo | Reduz overhead de I/O em até 50% |
| **5º** | Estratégia 5: Pipeline concorrente Produtor-Consumidor | Médio | Elimina tempo ocioso entre downloads |
