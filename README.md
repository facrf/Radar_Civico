# Radar Cívico 🛰️🇧🇷

**Plataforma de Alta Performance para Inteligência, Cruzamento e Auditoria Algorítmica de Dados Públicos Brasileiros.**

O **Radar Cívico** consolida bases de dados governamentais abertas em um Mini Data Lake SQLite local de alta velocidade, executando regras de auditoria contábil/física determinística e modelagem de grafos relacionais sem dependência de nuvem externa.

---

## 🏛️ Fontes Públicas Integradas

* **TSE (Tribunal Superior Eleitoral):** Candidaturas, fotos oficiais, patrimônio declarado e prestação de contas de campanhas (receitas e despesas).
* **Câmara dos Deputados (CEAP):** Notas fiscais e reembolsos da Cota Parlamentar.
* **PNCP (Portal Nacional de Contratações Públicas):** Contratos públicos e licitações de prefeituras e governos estaduais.
* **Querido Diário (Open Knowledge Brasil):** Atos, decretos e portarias de nomeações em diários oficiais municipais.
* **CNA / OAB (Conselho Federal da OAB):** Cadastro Nacional dos Advogados para checagem de regularidade e conflito de interesses.
* **Receita Federal:** Quadro de Sócios e Administradores (QSA) e situação cadastral de empresas.

---

## ⚙️ Arquitetura do Sistema

O projeto é estruturado em um **Cargo Workspace** modular em Rust e uma interface web reativa em **SvelteKit**:

```text
radar-civico/
├── crates/
│   ├── storage/    # Mini Data Lake em SQLite, PRAGMAs de alta performance, WAL e migrações SQL
│   ├── ingestion/  # Streams assíncronos (csv-async), normalização de CPF/CNPJ e scrapers (OAB/Diários)
│   ├── auditor/    # Regras determinísticas de detecção de fraudes, anomalias e conflito de interesses
│   ├── graph/      # Modelagem e travessia de redes (petgraph), caminhos até 3 graus e nós Hub
│   └── server/     # API HTTP Axum com compressão, CORS, graceful shutdown e servidor web SPA
├── tests/
│   └── e2e_pipeline.rs # Teste de integração ponta a ponta (Ingestão -> Auditoria -> Grafo -> API)
├── web/            # Interface SPA em SvelteKit, Tailwind CSS e Cytoscape.js
├── Dockerfile      # Empacotamento multi-stage de produção
└── docker-compose.yml
```

---

## 🔎 Regras do Motor de Auditoria

| Regra / Heurística | Descrição | Base Legal / Fonte |
| :--- | :--- | :--- |
| **Anomalia de Combustível** | Reembolsos da CEAP com volume faturado superior a 80L em veículo leve ou inconsistência com preço médio da ANP. | Limite físico de tanques |
| **Conflito de Interesses OAB** | Ocupantes de cargos do 1º escalão (Secretários, Diretores) com inscrição "Regular" ativa na OAB. | Art. 28 da Lei 8.906/94 |
| **Triangulação Doador x Contrato** | Empresas contratadas por órgãos públicos cujos sócios foram doadores de campanha em janela < 180 dias da posse. | TSE / PNCP / QSA |
| **Fornecedor Hub** | Empresas que concentram mais de 70% de repasses de candidatos de uma mesma coligação ou família política. | *Betweenness Centrality* |
| **Fornecedores Fantasmas** | Empresas abertas a menos de 30 dias da eleição ou com CNPJ inapto/baixado recebendo verbas eleitorais. | Receita Federal / TSE |
| **Ubiquidade Geotemporal** | Notas fiscais emitidas presencialmente pelo mesmo parlamentar em cidades distantes exigindo velocidade > 800 km/h. | Análise Geotemporal |

---

## 🚀 Como Executar

### 1. Pré-requisitos
* **Rust 1.75+** (`cargo`)
* **Node.js 18+** e `npm` (para o frontend)
* *(Opcional)* **Docker** e **Docker Compose**

### 2. Modo Servidor e Interface Web
Compile o frontend e inicie a API:
```bash
# 1. Compilar o frontend estático
cd web
npm install
npm run build
cd ..

# 2. Iniciar o servidor HTTP completo
cargo run -p server
```
Acesse a interface e a documentação no navegador:
👉 **`http://localhost:8080`**

### 3. Modo CLI de Auditoria
Para rodar a varredura das regras determinísticas diretamente no terminal:
```bash
cargo run -p server -- auditar
```

### 4. Execução em Containers Docker
```bash
docker compose up --build -d
docker compose ps
```

---

## 📡 Endpoints da API REST

* `GET /health` - Verificação de saúde da aplicação.
* `GET /api/v1/busca?q={termo}` - Busca textual instantânea unificada via FTS5 (políticos, doadores, empresas).
* `GET /api/v1/politico/{id}` - Dossiê consolidado: candidaturas, bens declarados, doadores e foto TSE em Base64.
* `GET /api/v1/grafo/{id}?grau=2` - Subgrafo relacional formatado para Cytoscape.js e Apache ECharts.
* `GET /api/v1/auditoria/alertas?ano=2024&severidade=CRITICA` - Ranking de anomalias com filtros por município/ano.
* `GET /api/v1/investigar/nomeacao/{doador_id}` - Disparo sob demanda na API do Querido Diário e CNA/OAB com checagem do Art. 28.

---

## 🧪 Suíte de Testes e Validação

```bash
# Executa todos os testes unitários do workspace Rust (52 testes)
cargo test --all

# Executa o teste de integração ponta a ponta
cargo test --test e2e_pipeline

# Valida tipagem TypeScript e build do frontend
cd web && npm run check && npm run build
```

---

## 📄 Licença
Distribuído sob licença aberta para fortalecimento da transparência pública e controle social.
