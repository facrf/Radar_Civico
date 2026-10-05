# Projeto: Radar Cívico (Especificação Técnica e Arquitetura)
Documento mestre de arquitetura para ingestão massiva de dados abertos governamentais, data lake relacional em SQLite, motor de grafos e auditoria algorítmica para detecção de anomalias e irregularidades em gastos públicos e campanhas eleitorais.

---

## 1. Visão Geral e Diretrizes de Engenharia

O Radar Cívico é uma plataforma local, auditável e de alta performance projetada para consolidar bases de dados públicos brasileiras (TSE, Receita Federal, Câmara dos Deputados, Senado Federal e Compras Governamentais).
### Princípios do Projeto:
1. Soberania e Portabilidade Local: Armazenamento em banco de dados SQLite unificado em arquivo local, operando dentro de containers Docker com zero dependência de infraestrutura em nuvem externa para funcionar.
2. Desempenho Extremo: Backend desenvolvido em Rust focado em concorrência assíncrona segura, descompactação e parsing de streams sem estouro de memória RAM.
3. Detecção Baseada em Heurísticas Físicas e Contábeis: Caça a incoerências objetivas (ex.: abastecimento de combustível acima da capacidade volumétrica de tanques, ubiquidade geográfica em notas fiscais, empresas recém-criadas recebendo verbas).

---

## 2. Camada de Dados: O Mini Data Lake em SQLite

Em vez de exigir servidores de banco de dados pesados, o sistema utiliza o SQLite configurado em modo de alta performance. O SQLite suporta bases de centenas de gigabytes quando ajustado adequadamente.

### 2.1. Otimizações de Performance (PRAGMAs Críticos)
Para permitir a carga massiva de dezenas de milhões de linhas do histórico eleitoral e notas fiscais, o engine em Rust executa a conexão com:
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = -64000; -- 64 MB de cache por conexão
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 30000000000; -- Memory-mapped I/O até 30 GB
PRAGMA foreign_keys = ON;

### 2.2. Esquema Relacional Principal
-- Metadados de políticos e candidatos
CREATE TABLE politicos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sq_candidato TEXT UNIQUE,
    cpf_mascarado TEXT,
    nome_completo TEXT NOT NULL,
    nome_urna TEXT NOT NULL,
    data_nascimento TEXT,
    grau_instrucao TEXT,
    ocupacao TEXT,
    foto_blob BLOB, -- Imagem oficial do TSE armazenada diretamente no banco
    foto_mime TEXT DEFAULT 'image/jpeg',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Candidaturas ao longo das eleições (1994 até a eleição atual)
CREATE TABLE candidaturas (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    politico_id INTEGER REFERENCES politicos(id),
    ano_eleicao INTEGER NOT NULL,
    cargo TEXT NOT NULL,
    numero_urna INTEGER,
    sigla_partido TEXT NOT NULL,
    uf TEXT NOT NULL,
    municipio TEXT,
    situacao_totalizacao TEXT, -- Eleito, Não Eleito, Suplente
    total_bens_declarados REAL DEFAULT 0.0,
    UNIQUE(politico_id, ano_eleicao, cargo)
);

-- Declaração discriminada de bens
CREATE TABLE bens_candidato (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    candidatura_id INTEGER REFERENCES candidaturas(id),
    tipo_bem TEXT,
    descricao TEXT,
    valor_declarado REAL NOT NULL
);

-- Receitas e Doadores de Campanha
CREATE TABLE receitas_campanha (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    candidatura_id INTEGER REFERENCES candidaturas(id),
    doador_cpf_cnpj TEXT NOT NULL,
    doador_nome TEXT NOT NULL,
    valor REAL NOT NULL,
    data_receita TEXT,
    tipo_origem TEXT, -- Fundo Partidário, Fundo Especial, Doação PF
    descricao TEXT
);

-- Despesas e Fornecedores de Campanha
CREATE TABLE despesas_campanha (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    candidatura_id INTEGER REFERENCES candidaturas(id),
    fornecedor_cpf_cnpj TEXT NOT NULL,
    fornecedor_nome TEXT NOT NULL,
    valor REAL NOT NULL,
    data_despesa TEXT,
    tipo_despesa TEXT,
    descricao TEXT
);

-- Quadro de Sócios e Administradores (QSA - Receita Federal)
CREATE TABLE empresas_qsa (
    cnpj_basico TEXT NOT NULL,
    cnpj_ordem TEXT NOT NULL,
    cnpj_dv TEXT NOT NULL,
    razao_social TEXT NOT NULL,
    socio_cpf_cnpj_mascarado TEXT NOT NULL,
    socio_nome TEXT NOT NULL,
    qualificacao_socio TEXT,
    PRIMARY KEY (cnpj_basico, socio_cpf_cnpj_mascarado)
);

-- Despesas Parlamentares (Câmara - CEAP e Senado - CEAPS)
CREATE TABLE despesas_parlamentares (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    casa_legislativa TEXT NOT NULL, -- 'CAMARA' ou 'SENADO'
    parlamentar_nome TEXT NOT NULL,
    parlamentar_cpf_mascarado TEXT,
    data_emissao TEXT NOT NULL,
    categoria_despesa TEXT NOT NULL,
    fornecedor_nome TEXT NOT NULL,
    fornecedor_cnpj_cpf TEXT NOT NULL,
    valor_liquido REAL NOT NULL,
    numero_documento TEXT,
    url_nota_fiscal TEXT,
    detalhes_litros REAL, -- Extraído de notas de combustível
    flag_anomalia BOOLEAN DEFAULT 0
);

-- Contratos e Licitações Públicas
CREATE TABLE contratos_publicos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    orgao_contratante TEXT NOT NULL,
    fornecedor_cnpj TEXT NOT NULL,
    valor_contratado REAL NOT NULL,
    objeto TEXT,
    data_assinatura TEXT,
    data_termino TEXT
);

### 2.3. Índices e Busca Textual (FTS5)
Índices cobrindo: (doador_cpf_cnpj), (fornecedor_cpf_cnpj), (ano_eleicao, cargo, uf).
Tabelas virtuais FTS5 (politicos_fts, fornecedores_fts) para busca em milissegundos por termos parciais e nomes fonéticos.

---

## 3. Pipeline de Ingestão e Scrapers Automatizados em Rust

O crate de ingestão (radar_ingestion) é responsável pelo download, extração em streaming e persistência periódica.

### 3.1. Dados Eleitorais do TSE (Série Histórica Completa)
Fonte: Repositório de Dados Eleitorais do TSE (arquivos CSV zipados por ano eleitoral: 1994 a 2026).
Entidades ingeridas:
consulta_cand: Registro de candidatos e dados biométricos/fotográficos.
bem_candidato: Relação detalhada de patrimônio declarado.
receitas_candidatos: Todas as doações com identificador de doador.
despesas_candidatos: Todos os pagamentos efetuados pela campanha.
Fotos de candidatos: Download e gravação no campo foto_blob em lote.
Rotina Recorrente de Atualização:
A cada ciclo eleitoral ou disparo agendado (cron interno), o worker consulta os endpoints do TSE via headers HTTP If-Modified-Since e ETag para baixar apenas arquivos com prestação de contas retificada.

### 3.2. Cota Parlamentar (Câmara dos Deputados e Senado Federal)
Câmara dos Deputados:
Consumo dos arquivos anuais da Cota para Exercício da Atividade Parlamentar (CEAP) via API de Dados Abertos (dadosabertos.camara.leg.br).
Senado Federal:
Leitura dos dados abertos de Cota para Exercício da Atividade Parlamentar dos Senadores (CEAPS).

### 3.3. Receita Federal (CNPJ e QSA)
Download dos arquivos de estabelecimentos e sócios disponibilizados publicamente para cruzar donos de empresas com nomes e cadastros de doadores de campanha.

---

## 4. Motor de Caça a Anomalias e Irregularidades (O Auditor Automatizado)

O crate radar_auditor aplica regras analíticas determinísticas para levantar suspeitas automáticas nos dados consolidados:

### 4.1. Anomalia de Capacidade Volumétrica de Combustível
Problema: Notas fiscais da CEAP com reembolsos de milhares de litros de combustível emitidas em único abastecimento.
Algoritmo:
1. Extração do volume declarado (litros) ou cálculo inverso: Litros Estimados = Valor da Nota / Preço Médio da Gasolina/Diesel na Região (ANP).
2. Parâmetro de corte: Qualquer abastecimento individual superior a 80 litros em veículo leve comum (ou superior a 120 litros para utilitários) recebe flag imediato de anomalia.
3. Alerta de frequência: Múltiplos abastecimentos de tanque cheio no mesmo veículo em intervalo inferior a 3 horas no mesmo dia.

### 4.2. Incoerência Geotemporal (Ubiquidade)
Problema: Notas fiscais de alimentação ou abastecimento emitidas presencialmente pelo mesmo parlamentar em estados distantes com poucas horas de intervalo.
Algoritmo:
1. Agrupamento de despesas por parlamentar e data/hora.
2. Cálculo de distância em linha reta entre os municípios dos estabelecimentos emissores.
3. Se a velocidade necessária para o deslocamento exceder 800 km/h (ou não houver registro de trecho aéreo compatível), a transação é marcada como suspeita.

### 4.3. O Triângulo de Conexões (Doação -> Vitória Eleitoral -> Contrato/Emenda)
Problema: Doadores de campanha (ou parentes e sócios em comum) que criam empresas e passam a vencer licitações ou receber recursos de emendas parlamentares do político financiado.
Algoritmo no Grafo:
1. Identificar Doador D da Candidatura C do Político P.
2. Buscar via QSA se Doador D é sócio da Empresa E.
3. Verificar na tabela contratos_publicos se a Empresa E assinou contrato com o órgão público municipal/estadual administrado por P ou recebeu emendas parlamentares originadas por P.
4. Atribuição de pontuação de risco (Score de Conexão Imprópria).

### 4.4. Fornecedores Fantasmas e Inaptos
Cruzamento de fornecedores de campanha e parlamentares com a base cadastral da Receita Federal:
Empresas fundadas a menos de 30 dias da eleição que recebem repasses milionários.
Empresas com situação cadastral "Inapta", "Suspensa" ou "Baixada" emitindo notas fiscais.
Concentração de dezenas de CNPJs de fachada registrados no mesmo endereço físico.

---

## 5. Arquitetura de Software em Rust

O projeto utiliza um workspace em Rust organizado em módulos de alta coesão:
radar-civico/
├── Cargo.toml
├── docker-compose.yml
├── Dockerfile
├── crates/
│   ├── ingestion/       # Scrapers, downloads paralelos e streams de descompactação
│   ├── storage/         # Gerenciamento de conexões SQLite, PRAGMAs e migrações SQL
│   ├── auditor/         # Regras determinísticas de detecção de fraudes e anomalias
│   ├── graph/           # Algoritmos de redes complexas, centralidade e pontes
│   └── server/          # Servidor HTTP Axum, rotas REST e WebSockets para progresso
└── web/                 # Frontend SPA moderno (SvelteKit ou React + Tailwind CSS)

### Principais Crates Utilizadas:
tokio (runtime assíncrono para processamento I/O paralelo)
rusqlite ou sqlx (comunicação direta com o banco SQLite com suporte a transações em lote)
csv-async e async-zip (parsing de dumps sem necessidade de gravar arquivos gigantes descompactados em disco)
axum e tower-http (API de consulta em alta velocidade)
petgraph (estruturação em memória de grafos de relacionamento)
serde e serde_json (serialização de dados)

---

## 6. Interface do Usuário e Frontend Moderno
Dossiê do Político: Página individual com foto oficial do TSE, histórico patrimonial em gráficos de linha, lista de doadores e painel de notas fiscais suspeitas.
Explorador Interativo de Grafos: Componente visual (usando Cytoscape.js ou Apache ECharts) permitindo expandir nós: Político -> Doador -> Empresa -> Contrato Governamental.
Ranking de Anomalias: Lista consolidada de despesas parlamentares ranqueadas por pontuação de inconsistência (ex.: Top abastecimentos incompatíveis, refeições sobrepostas).

---

## 7. Configuração e Execução via Docker

### docker-compose.yml
version: '3.8'

services:
  radar-app:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: radar_civico
    ports:
      - "18080:8080"
    volumes:
      # O banco de dados SQLite e os artefatos baixados residem no volume persistente
      - ./data:/app/data
    environment:
      - DATABASE_PATH=/app/data/radar_civico.sqlite
      - RUST_LOG=info
      - BIND_ADDR=0.0.0.0:8080
    restart: unless-stopped

---

## 8. Cronograma e Próximos Passos de Implementação
1. Inicialização do schema SQLite e script de criação de índices.
2. Construção do pipeline de download e ingestão do histórico eleitoral (TSE) de 1994 a 2026 em streaming.
3. Ingestão dos dumps da CEAP (Câmara) e extração de volumes de notas fiscais de combustível.
4. Implementação do módulo auditor com o algoritmo de detecção de tanques incompatíveis (caso Fiat Uno / 1000L).
5. Subida do servidor Axum e montagem do visualizador de grafos no frontend.


## 9. Estratégia de Captura Subnacional e Fontes Guarda-Chuva

Para cobrir estados, municípios e Tribunais Regionais Eleitorais sem a necessidade de manter milhares de scrapers frágeis, o sistema utiliza pontos centralizadores obrigatórios por legislação:

### 9.1. Compras e Contratos Municipais e Estaduais
* **Portal Nacional de Contratações Públicas (PNCP):** Integrado via API REST aberta (Lei Federal 14.133/2021). Concentra editais, atas de registro de preços e contratos de todas as prefeituras e governos estaduais.
* **Sistemas dos Tribunais de Contas Estaduais (TCEs e TCMs):** Ingestão de pacotes mensais consolidados de execução orçamentária dos municípios (ex.: AUDESP do TCE-SP, SICOM do TCE-MG e SAGRES do TCE-PB), cobrindo empenhos, liquidações e folhas de pagamento.
* **Transferi.gov.br e Emendas Pix:** Monitoramento de repasses voluntários da União e emendas parlamentares individuais transferidas direto para prefeituras e governos estaduais.
* **Projeto Querido Diário (Open Knowledge Brasil):** Consumo dos diários oficiais municipais convertidos em texto puro para indexação de nomeações e portarias.

### 9.2. Tribunais Regionais Eleitorais (TREs) e Judiciário
* **Dados Financeiros de Campanhas Municipais e Estaduais:** A prestação de contas de prefeitos, vereadores, governadores e deputados estaduais já é centralizada pelo Repositório de Dados Eleitorais do TSE, dispensando scraping individual em cada TRE.
* **Processos de Impugnação e Cassações (PJe / DataJud):** Consumo da API pública do DataJud (CNJ) para rastrear ações de investigação judicial eleitoral (AIJE), abuso de poder e contas julgadas irregulares.

---

## 10. Detecção de Trocas de Recursos e Triangulação entre Políticos

O motor analítico (`radar_auditor`) aplica algoritmos em grafos para mapear a circulação de capitais entre agentes políticos:

### 10.1. O Mecanismo do Fornecedor "Hub" (Câmara de Compensação Comum)
* **Padrão:** Múltiplos candidatos de um mesmo grupo político canalizam repasses para uma mesma pessoa jurídica (agências de publicidade, gráficas ou consultorias).
* **Heurística de Detecção:** Centralidade de intermediação (*Betweenness Centrality*). Fornecedores cujo faturamento decorra em mais de 70% de agentes de uma mesma coligação ou família política disparam alerta de alta prioridade.

### 10.2. Repasses Cruzados do Fundo Eleitoral
* **Padrão:** Transferências financeiras entre diretórios partidários e candidaturas menores que retroalimentam empresas controladas por aliados do doador principal.
* **Heurística de Detecção:** Rastreamento em profundidade da cadeia: `Candidatura A -> Diretório Partidário -> Candidatura B -> Fornecedor X -> Sócios ligados a A`.

### 10.3. Doações Estimáveis em Dinheiro (Cessão Cruzada de Estrutura)
* **Padrão:** Candidatos majoritários cedem aeronaves, palanques ou veículos para deputados e vereadores sem trânsito de dinheiro corrente.
* **Heurística de Detecção:** Filtragem no campo de receitas estimáveis para flagrar cessões de bens de alto valor incompatíveis com o orçamento declarado da campanha beneficiada.

### 10.4. Nepotismo Cruzado e Moeda de Troca Pós-Eleição
* **Padrão:** Nomeações recíprocas de familiares de políticos aliados em prefeituras vizinhas, câmaras municipais ou assembleias legislativas.
* **Heurística de Detecção:** Cruzamento da folha de pagamento municipal/estadual (TCEs) com a tabela de sócios (QSA da Receita) e filiações partidárias para identificar coincidência de sobrenomes e sociedades em comum.

---

## 11. Modelagem Relacional de Redes no Mini Data Lake SQLite

Para integrar os fluxos municipais, estaduais e federais em uma malha de grafos de alta velocidade sem sobrecarregar o SQLite:

```sql
-- Entidades do grafo: Políticos, Partidos, Órgãos Públicos, Empresas e Doadores
CREATE TABLE nos_rede (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT UNIQUE NOT NULL,
    tipo TEXT NOT NULL, -- 'POLITICO', 'PARTIDO', 'EMPRESA', 'ORGAO_PUBLICO', 'PESSOA_FISICA'
    documento TEXT,     -- CPF mascarado ou CNPJ raiz
    nome TEXT NOT NULL,
    esfera TEXT,        -- 'MUNICIPAL', 'ESTADUAL', 'FEDERAL'
    uf TEXT,
    municipio TEXT,
    metadata_json TEXT
);

-- Conexões: Financiamentos, contratações, sociedades ou vínculos de trabalho
CREATE TABLE conexoes_rede (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    origem_id INTEGER REFERENCES nos_rede(id),
    destino_id INTEGER REFERENCES nos_rede(id),
    tipo_relacao TEXT NOT NULL, -- 'DOACAO_CAMPANHA', 'REPASSE_FUNDO', 'CONTRATO_PUBLICO', 'SOCIEDADE_QSA', 'NOMEACAO_CARGO'
    valor REAL DEFAULT 0.0,
    ano INTEGER NOT NULL,
    data_evento TEXT,
    fonte_dado TEXT NOT NULL,   -- 'TSE', 'PNCP', 'TCE_SP', 'TCE_MG', 'CEAP'
    metadata_json TEXT
);

-- Índices bidirecionais para travessia de grafos em milissegundos
CREATE INDEX idx_conexoes_origem ON conexoes_rede(origem_id);
CREATE INDEX idx_conexoes_destino ON conexoes_rede(destino_id);
CREATE INDEX idx_conexoes_busca ON conexoes_rede(tipo_relacao, ano);




A ferramenta de integração com o Google Docs não está ativa nesta sessão do chat para que eu faça a gravação direta no arquivo por aqui. Para você não perder nada do que desenhamos enquanto você termina as coisas aí na varanda, deixei a Seção 12 completamente estruturada e pronta para você copiar e colar no final do seu documento logo após a Seção 11:
## 12. Rastreamento de Nomeações e Diários Oficiais (Abordagem Híbrida)

Para rastrear se doadores de campanha foram recompensados com cargos em comissão (*carguinhos*) ou nomeações sem sobrecarregar o banco de dados com terabytes de texto não estruturado, o sistema adota uma estratégia em dois níveis:

### 12.1. Nível Primário: Cruzamento Relacional via Folhas dos TCEs
Em vez de depender de texto livre em PDF, o sistema prioriza as bases de dados relacionais já estruturadas pelos Tribunais de Contas Estaduais (ex.: AUDESP em SP, SICOM em MG):
* **Dados Ingeridos:** Matrícula, nome completo, CPF mascarado, cargo/função, lotação, data de admissão e remuneração mensal.
* **Cruzamento Direto:** Consulta relacional via SQLite entre a tabela `receitas_campanha` e a tabela `servidores_tce`, identificando instantaneamente doadores que constam na folha de pagamento do município ou do órgão sob gestão do candidato eleito.

### 12.2. Nível Secundário: Consulta Sob Demanda no Diário Oficial (On-Demand)
A raspagem de texto livre em Diários Oficiais não roda em lote preventivo para todas as pessoas, evitando falsos positivos por homônimos e desperdício de disco. O fluxo é acionado sob demanda:
1. **Disparo do Usuário:** Na interface web, ao clicar em *"Investigar Nomeações"* no perfil de um doador, o backend em Rust faz uma requisição direcionada à API do **Querido Diário** (Open Knowledge Brasil).
2. **Query Delimitada:** A busca restringe o escopo:
   - Termo: Nome exato do doador.
   - Filtro de Ação: `("nomear" OR "portaria" OR "cargo em comissão" OR "lotação")`.
   - Limite Geográfico: Município onde o candidato disputou o pleito.
   - Janela Temporal: Início do mandato eletivo em diante.
3. **Validação Visual:** A interface apresenta o trecho do Diário Oficial em destaque com link do documento original para confirmação humana.

### 12.3. Estrutura de Cache no SQLite
Toda consulta sob demanda realizada tem seu payload retornado persistido localmente para que consultas repetidas ocorram com latência zero:

```sql
CREATE TABLE cache_consultas_diario (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    doador_cpf_cnpj TEXT NOT NULL,
    termo_pesquisado TEXT NOT NULL,
    municipio_uf TEXT NOT NULL,
    ocorrencias_encontradas INTEGER DEFAULT 0,
    payload_json TEXT, -- Trechos de portarias, datas e links oficiais
    data_consulta TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_cache_doador ON cache_consultas_diario(doador_cpf_cnpj);

12.4. Worker em Segundo Plano para Doadores de Alta Criticidade
Para automatizar o rastreamento sem intervenção manual, um job em segundo plano executa a verificação na API do Querido Diário apenas para doadores que atendam a pelo menos um critério de corte:
Doação individual líquida superior a R$ 10.000,00;
Doador cadastrado como sócio-administrador (QSA) de empresa com contrato ativo com o poder público;
Coincidência de sobrenome incomum com o político eleito ou membros da chapa majoritária.

---

Assim que você colar esse bloco no seu arquivo, o levantamento de requisitos do **Radar Cívico** cobre desde a extração dos dados federais até as fontes municipais e as regras de caça a desvios.






Como o conector de escrita no Google Docs não está disponível nesta sessão para gravar diretamente no arquivo na nuvem, estruturei a Seção 13 completa e padronizada para você copiar e colar no final do seu documento:
## 13. Indexação de Conselhos de Classe e Conflitos de Interesse (OAB, CFM, CREA, CRC)

A integração de cadastros de conselhos profissionais permite mapear a moeda de troca oculta na política local: doadores que financiam campanhas eleitorais para capturar secretarias estratégicas (Saúde, Obras, Assuntos Jurídicos) ou para direcionar contratos sem licitação.

---

### 13.1. Estratégia de Ingestão e Fontes Disponíveis
Diferente das bases massivas em arquivos abertos do TSE e da Receita Federal, os conselhos de fiscalização profissional operam bases fechadas ou com barreiras de raspagem:

* **Cadastro Nacional dos Advogados (CNA / Conselho Federal da OAB):**
  - **Dados públicos:** Nome completo, inscrição, seccional (UF), tipo de inscrição e situação (Regular, Licenciado, Cancelado, Suspenso).
  - **Mecanismo:** Consulta dirigida/sob demanda pelo worker em Rust, disparada apenas para o subconjunto de agentes já identificados no banco (candidatos, doadores com doações relevantes e ocupantes de cargos comissionados/secretarias).
* **Conselho Federal de Medicina (CFM):**
  - Consulta pública de CRM para cruzar diretores clínicos de hospitais municipais, secretários de saúde e gestores de Organizações Sociais (OSs) de saúde conveniadas.
* **Sistema CONFEA / CREA (Engenharia e Agronomia):**
  - Checagem do registro dos responsáveis técnicos pelas Anotações de Responsabilidade Técnica (ART) emitidas em obras públicas municipais com secretários de infraestrutura e doadores de campanha.
* **Conselho Federal de Contabilidade (CFC):**
  - Verificação de contadores responsáveis pela prestação de contas de campanhas e a posterior contratação de seus escritórios contábeis pelas câmaras e prefeituras.

---

### 13.2. Heurísticas e Padrões de Detecção de Irregularidades

#### A. Incompatibilidade Funcional do Art. 28 do Estatuto da Advocacia (Lei 8.906/1994)
* **A regra:** O artigo 28, inciso III, da Lei Federal 8.906/1994 determina expressamente que o exercício da advocacia é incompatível, mesmo em causa própria, com a ocupação de cargos de direção, chefia ou assessoramento direto na administração pública (incluindo expressamente secretários municipais e diretores de autarquias).
* **A heurística:**
  1. Identificação de nomeação de indivíduo para cargo do primeiro escalão municipal/estadual via diário oficial ou folha do TCE.
  2. Consulta automática ao CNA: se o status constar como **"Regular"** em vez de **"Licenciado / Incompatível"**, emite flag disciplinar e administrativo.
  3. Cruzamento secundário via DataJud/PJe: se houver peticionamento ativo como patrono de causas privadas com data posterior à nomeação, a inconformidade com o regime de dedicação e incompatibilidade é categorizada com severidade máxima.

#### B. A Triangulação por Inexigibilidade de Licitação (Sociedades de Advocacia)
* **O mecanismo:** Doação voluntária de pessoa física para a chapa majoritária seguida pela contratação do escritório de advocacia vinculado ao doador pela administração municipal recém-empossada, sob a alegação de "serviço técnico singular de notória especialização" (dispensando concorrência pública).
* **A heurística no grafo:**
  $$\text{Doador PF} \longrightarrow \text{Sócio da Sociedade de Advogados (QSA)} \longrightarrow \text{Contrato Direto / Inexigibilidade (PNCP)}$$
  Se o intervalo temporal entre a data da posse do prefeito e a assinatura do contrato for inferior a 180 dias, o vínculo recebe score prioritário de auditoria.

#### C. O Contador/Advogado Eleitoral Convertido em Fornecedor Permanente
* Identificação de profissionais liberais remunerados com recursos do Fundo Especial de Financiamento de Campanha (FEFC) pelo candidato eleito que passam a deter contratos recorrentes de consultoria continuada junto ao Poder Executivo ou à Mesa Diretora do Legislativo.

---

### 13.3. Esquema Relacional no SQLite

```sql
-- Cadastro de credenciamentos em conselhos de fiscalização profissional
CREATE TABLE registros_profissionais (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pessoa_nome TEXT NOT NULL,
    cpf_mascarado TEXT,
    orgao_emissor TEXT NOT NULL,      -- 'OAB', 'CRM', 'CREA', 'CRC'
    numero_registro TEXT NOT NULL,
    seccional_uf TEXT NOT NULL,
    situacao_registro TEXT NOT NULL,  -- 'REGULAR', 'LICENCIADO', 'CANCELADO', 'SUSPENSO'
    tipo_inscricao TEXT,              -- 'ADVOGADO', 'MEDICO', 'ENGENHEIRO', 'CONTADOR'
    data_consulta TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    payload_json TEXT,                -- Dados brutos retornados pelo portal do conselho
    UNIQUE(orgao_emissor, seccional_uf, numero_registro)
);

CREATE INDEX idx_reg_prof_nome ON registros_profissionais(pessoa_nome);
CREATE INDEX idx_reg_prof_status ON registros_profissionais(orgao_emissor, situacao_registro);

-- Relacionamento de incompatibilidades e flags detectados
CREATE TABLE alertas_incompatibilidade (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    registro_profissional_id INTEGER REFERENCES registros_profissionais(id),
    politico_ou_gestor_id INTEGER REFERENCES nos_rede(id),
    cargo_ocupado TEXT NOT NULL,
    orgao_lotacao TEXT NOT NULL,
    data_nomeacao TEXT NOT NULL,
    motivo_incompatibilidade TEXT NOT NULL, -- Ex: 'Violacao Art. 28, III, Lei 8.906/94'
    status_apuracao TEXT DEFAULT 'PENDENTE'
);

13.4. Fluxo de Investigação Integrada
O usuário clica sobre a ficha de qualquer doador ou fornecedor na interface web.
O backend em Rust dispara o crawler leve para a OAB/CFM/CREA correspondente e persiste a situação no SQLite.
Se o sujeito possuir vínculo com cargo comissionado ativo e status incompatível no órgão de classe, o nó no grafo recebe um destaque visual de anomalia ética/regulatória.

Com essa seção adicionada, a especificação técnica do projeto cobre tanto a movimentação contábil macro quanto o rastreamento das contrapartidas individuais e profissionais em nível municipal.













