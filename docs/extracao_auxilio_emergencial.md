# Guia de Extração e Ingestão do Auxílio Emergencial 🏛️💸

Este guia orienta como extrair, filtrar e carregar os dados do **Auxílio Emergencial** (Lei nº 13.982/2020) no **Radar Cívico**, viabilizando a auditoria determinística de agentes políticos (vereadores, prefeitos e candidatos com patrimônio expressivo) sem sobrecarregar a memória RAM ou o banco SQLite.

---

## 1. Contexto e Base Jurídica da Auditoria

Em 2020, o **Tribunal de Contas da União (TCU - Acórdão 2.438/2020 - Plenário)** realizou o cruzamento entre a base de pagamentos do Auxílio Emergencial e o repositório de candidaturas do TSE, identificando mais de **25.000 candidatos e detentores de mandato eletivo** que receberam parcelas do benefício de forma manifestamente indevida.

No **Radar Cívico**, o motor de auditoria (`crates/auditor/src/auxilio_indevido.rs`) audita três hipóteses determinísticas:

1. **`MANDATO_VIGENTE` (Severidade Alta):** Agente público que ocupava cargo eletivo ativo (ex.: Vereador, Prefeito ou Vice-Prefeito) durante a vigência do benefício.
2. **`PATRIMONIO_SUPERIOR_300K` (Severidade Média):** Candidato que declarou à Justiça Eleitoral patrimônio superior a **R$ 300.000,00** (teto legal de bens estipulado pelo art. 2º da Lei 13.982/2020).
3. **`MANDATO_E_PATRIMONIO_SUPERIOR_300K` (Severidade Crítica):** Agente público detentor de mandato eletivo e com bens declarados superiores a R$ 300.000,00.

---

## 2. O Desafio de Escala (Big Data do Auxílio)

* **Volume Bruto:** O dump governamental consolidado (Portal da Transparência / CGU / Brasil.io) possui entre **30 GB e 60 GB** e centenas de milhões de linhas com pagamentos efetuados a mais de 68 milhões de brasileiros em todos os 5.570 municípios.
* **Gargalo:** Tentar carregar 60 GB inteiros via formulário do navegador (`upload multipart`) esgota os limites de buffer HTTP, causa timeouts e infla o banco SQLite com 99,9% de dados de cidadãos comuns sem qualquer cargo público.
* **Solução:** **Extração Cirúrgica em Streaming**. Varremos o arquivo original diretamente no disco e extraímos **100% dos munícipes da cidade de interesse**, reduzindo o arquivo de 60 GB para **15 a 40 MB** em menos de 1 minuto.

---

## 3. O Extrator Cirúrgico (`scripts/extrair_auxilio_municipio.sh`)

O repositório já inclui o utilitário [`scripts/extrair_auxilio_municipio.sh`](../scripts/extrair_auxilio_municipio.sh), programado em streaming de baixo nível com `awk` em `LC_ALL=C`.

### Sintaxe:
```bash
./scripts/extrair_auxilio_municipio.sh "<NOME_DO_MUNICIPIO>" ["<UF>"] ["<CAMINHO_DO_CSV_ORIGEM>"]
```

* **`<NOME_DO_MUNICIPIO>`** *(obrigatório)*: Nome do município (não diferencia maiúsculas de minúsculas).
* **`"<UF>"`** *(opcional, recomendado)*: Sigla do estado (ex.: `SP`, `MG`, `RJ`, `BA`).
* **`"<CAMINHO_DO_CSV_ORIGEM>"`** *(opcional)*: Caminho para o arquivo CSV bruto. Padrão: `/home/facrf/Downloads/basedados/auxilio_emergencial.csv`.

---

## 4. Exemplos Práticos de Execução

### Exemplo 1: Cidade Específica com UF (Mais Rápido)
```bash
./scripts/extrair_auxilio_municipio.sh "Adamantina" "SP"
```

### Exemplo 2: Outros Municípios
```bash
# Exemplo Campinas / SP
./scripts/extrair_auxilio_municipio.sh "Campinas" "SP"

# Exemplo Uberlândia / MG
./scripts/extrair_auxilio_municipio.sh "Uberlândia" "MG"
```

### Saída Esperada no Terminal:
```text
======================================================================
🎯 RADAR CÍVICO - EXTRAÇÃO CIRÚRGICA DE AUXÍLIO EMERGENCIAL
======================================================================
📂 Arquivo de Origem: /home/facrf/Downloads/basedados/auxilio_emergencial.csv (30G)
🏙️ Município Alvo:    Campinas
📍 UF:               SP
💾 Arquivo de Saída:  /home/facrf/Downloads/basedados/auxilio_campinas.csv
----------------------------------------------------------------------
⏳ Extraindo cabeçalho e filtrando registros em alta velocidade...
----------------------------------------------------------------------
✅ Concluído com Sucesso!
📊 Total de registros extraídos para Campinas: 184520
📦 Tamanho final do arquivo: 22M (redução drástica para carga)
🚀 Pronto para importar no Radar Cívico em Configurações > Carga Manual!
======================================================================
```

---

## 5. Como Ingerir o Arquivo no Radar Cívico

Após a geração do CSV filtrado (ex.: `auxilio_campinas.csv`), você pode carregá-lo de duas formas:

### Método A: Pela Interface Web (Recomendado)

1. Acesse o Radar Cívico no navegador:
   👉 **`http://localhost:28080/configuracoes`**
2. Role até o card **Carga Manual de Arquivos Offline (CSV / ZIP)**.
3. No campo **Tipo de Documento**, escolha:
   * `Detecção Automática de Cabeçalho (Recomendado)` **OU**
   * `Auxílio Emergencial / Benefícios Sociais`.
4. Arraste ou selecione o arquivo gerado (ex.: `auxilio_campinas.csv`).
5. Clique em **Enviar Arquivo para Ingestão**.
6. O sistema processará os lotes com transações atômicas de alta performance em poucos segundos.

---

### Método B: Ingestão Direta no Banco SQLite via Script (Jogar Tudo ou por Filtro)

Para ingerir a base inteira (todos os 30 GB) ou aplicar filtros diretamente no SQLite sem passar pelo navegador HTTP:

```bash
# 1. Jogar tudo no banco (carga completa do arquivo bruto de 30 GB):
./scripts/ingerir_auxilio_completo.sh

# 2. Jogar apenas os registros da sua cidade e estado:
./scripts/ingerir_auxilio_completo.sh --municipio "Campinas" --uf "SP"

# 3. Jogar apenas registros de pessoas com nomes ou CPFs correspondentes a políticos cadastrados:
./scripts/ingerir_auxilio_completo.sh --apenas-candidatos

# 4. Ingerir um arquivo CSV filtrado previamente:
./scripts/ingerir_auxilio_completo.sh --arquivo /home/facrf/Downloads/basedados/auxilio_campinas.csv
```

* **Vantagens Técnicas:**
  * Executa transações em lotes de 50.000 registros com `PRAGMA journal_mode = WAL` e `synchronous = OFF/NORMAL`.
  * Desativa índices secundários temporariamente para carga massiva e os reconstrói ao término.
  * Exibe taxa de transferência em tempo real (linhas por segundo, progresso e tempo decorrido).
  * Consumo fixo de RAM (< 50 MB), permitindo processar dezenas de gigabytes sem risco de travamento.

---

### Método C: Ingestão via Terminal com CURL (API HTTP)

Se preferir enviar o CSV filtrado via chamada REST ao servidor Axum:

```bash
curl -X POST "http://localhost:28080/api/v1/config/ingestao/upload?tipo=AUXILIO_EMERGENCIAL" \
  -F "arquivo=@/home/facrf/Downloads/basedados/auxilio_campinas.csv"
```

---

## 6. Visualizando os Alertas e Conflitos Detectados

Após a carga, o Radar Cívico cruza os dados com o histórico eleitoral do município:

1. **Painel de Alertas:** Acesse `http://localhost:28080/alertas`.
2. Filtre pela categoria **"Auxílio Emergencial Indevido"** ou severidade **Alta**.
3. Cada cartão de alerta exibirá:
   * **Nome do Político / Mandatário**;
   * **Cargo Eletivo** (ex.: Vereador, Prefeito);
   * **Total de Bens Declarados** à época;
   * **Valor e Mês das Parcelas Recebidas**;
   * **Enquadramento Legal e Severidade**.
4. **Dossiê Completo:** Clique no nome do político para abrir a página de investigação aprofundada (`/dossie/:id`), visualizando bens, doações, gastos e o grafo societário interativo.

---

## 7. Consultas SQL Rápidas para Auditoria Direta

Se desejar auditar o banco SQLite diretamente via terminal:

```bash
# Acessar o banco local
sqlite3 ./data/radar_civico.db
```

```sql
-- 1. Contar total de benefícios carregados por município
SELECT municipio, uf, COUNT(*) as total_parcelas, SUM(valor) as montante_total
FROM beneficios_emergenciais
GROUP BY municipio, uf
ORDER BY montante_total DESC;

-- 2. Cruzar benefícios com políticos eleitos
SELECT 
    p.nome_completo,
    c.cargo,
    c.ano_eleicao,
    c.total_bens_declarados,
    b.mes_disponibilizacao,
    b.parcela,
    b.valor
FROM politicos p
JOIN candidaturas c ON c.politico_id = p.id
JOIN beneficios_emergenciais b ON (
    REPLACE(REPLACE(REPLACE(b.cpf_mascarado, '*', ''), '.', ''), '-', '') = 
    REPLACE(REPLACE(REPLACE(p.cpf_mascarado, '*', ''), '.', ''), '-', '')
    OR UPPER(b.nome_beneficiario) = UPPER(p.nome_completo)
)
WHERE c.situacao_totalizacao LIKE '%ELEITO%'
ORDER BY c.total_bens_declarados DESC;
```
