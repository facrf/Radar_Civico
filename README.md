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
* **Auxílio Emergencial (CGU / Portal da Transparência):** Histórico de pagamentos de benefícios sociais com auditoria determinística de agentes políticos. *(Consulte o [Guia de Extração do Auxílio](docs/extracao_auxilio_emergencial.md))*.

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
├── scripts/        # Utilitários de linha de comando de alta velocidade (ex.: extrator cirúrgico de auxílio)
├── docs/           # Guias de auditoria, ingestão e documentação do sistema
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
| **Auxílio Emergencial Indevido** | Recebimento de benefício social por detentor de mandato ativo (Prefeito/Vereador) ou patrimônio > R$ 300.000,00. | Lei 13.982/2020 e TCU Acórdão 2.438/2020 |
| **Evolução Patrimonial Abrupta** | Crescimento patrimonial acima de 300% com saldo absoluto superior a R$ 200.000 entre pleitos consecutivos. | TSE / DivulgaCandContas |
| **Doador Incompatível** | Cidadão beneficiário de programas sociais de vulnerabilidade doando valores substanciais para campanhas. | CGU / TSE Prestação de Contas |
| **Conluio em Licitações** | Empresas concorrendo ou contratadas pelo mesmo órgão público que compartilham o mesmo quadro societário. | PNCP / Receita Federal QSA |
| **Capital Social Ínfimo vs Faturamento** | Empresas com capital social $\le$ R$ 5.000 faturando mais de R$ 100.000 em órgãos públicos ou cotas parlamentares. | CEAP / PNCP / Receita Federal |

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

### 4. Execução com Docker Compose (Porta 28080)

O sistema já vem configurado por padrão para subir na porta **`28080`** no host (mapeando internamente para a porta `8080` do container):

```yaml
# docker-compose.yml
services:
  app:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: radar_civico
    ports:
      - "28080:8080"
    volumes:
      - ./data:/app/data
    environment:
      - DATA_DIR=/app/data
      - PORT=8080
      - WEB_DIR=/app/web/build
      - RUST_LOG=info
    restart: unless-stopped
```

Para construir a imagem e iniciar em segundo plano:
```bash
# Iniciar o container
docker compose up --build -d

# Verificar status e logs
docker compose ps
docker compose logs -f

# Testar endpoint de saúde
curl -i http://localhost:28080/health
```

Acesse no navegador: 👉 **`http://localhost:28080`**

---

### 5. Implantação no Portainer (Stack Compose)

Para implantar via **Portainer**:
1. Acesse o painel do Portainer (`http://seu-servidor:9000` ou `9443`).
2. Navegue até **Stacks** ➔ **Add stack**.
3. Defina o nome (ex: `radar-civico`) e selecione o método **Web editor**.
4. Cole o modelo YAML abaixo:

```yaml
version: '3.8'

services:
  radar_civico:
    image: radar_civico:latest # ou informe o caminho da imagem no seu registry
    # Caso use repositório Git diretamente no Portainer:
    # build:
    #   context: .
    #   dockerfile: Dockerfile
    container_name: radar_civico
    restart: unless-stopped
    ports:
      - "28080:8080"
    volumes:
      - radar_civico_data:/app/data
    environment:
      - DATA_DIR=/app/data
      - PORT=8080
      - WEB_DIR=/app/web/build
      - RUST_LOG=info
    healthcheck:
      test: ["CMD-SHELL", "curl -f http://localhost:8080/health || exit 1"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 15s

volumes:
  radar_civico_data:
    driver: local
```

5. Clique em **Deploy the stack**.
6. Acesse a aplicação em **`http://<ip-do-servidor>:28080`**.

---

## 📡 Endpoints da API REST

* `GET /health` - Verificação de saúde da aplicação (utilizado pelo healthcheck do Docker).
* `GET /api/v1/busca?q={termo}` - Busca unificada de alta performance cobrindo:
  - Políticos (`politicos_fts` e identificadores TSE).
  - Fornecedores de campanha e CEAP (`fornecedores_fts`).
  - Doadores eleitorais (`receitas_campanha`).
  - Quadro de Sócios e Administradores da Receita Federal (`empresas_qsa`) com suporte a CPF completo (11 dígitos), miolo de 6 dígitos, CPF mascarado (`***123456**`) e CNPJ de empresas.
* `GET /api/v1/politico/{id}` - Dossiê consolidado: candidaturas, bens declarados, doadores e foto oficial do TSE.
* `GET /api/politicos?ano=2022&cargo=PRESIDENTE` - Listagem e busca de políticos com filtro multi-ano (2022/2024), ordenação por escalão e cargos executivos (Presidentes, Governadores, Prefeitos).
* `GET /api/politicos/duplicados/resumo` - Diagnóstico de cadastros redundantes multi-pleito acelerado por cache em memória TTL.
* `POST /api/politicos/duplicados/mesclar-automatico?limite={N}` - Unificação em lote atômica de cadastros redundantes por nome civil e data de nascimento.
* `GET /api/v1/grafo/{id}?grau=2` - Subgrafo relacional formatado para Cytoscape.js e Apache ECharts.
* `GET /api/v1/auditoria/alertas?ano=2024&severidade=CRITICA` - Ranking de anomalias com indicação de fontes primárias, filtros e sincronização em tempo real.
* `GET /api/v1/investigar/nomeacao/{doador_id}` - Disparo sob demanda na API do Querido Diário e CNA/OAB com checagem do Art. 28 da Lei 8.906/94.

---

## 🧪 Suíte de Testes e Validação

```bash
# Executa todos os testes unitários e de integração do workspace Rust (> 90 testes)
cargo test --all

# Executa as suítes de pipeline ponta a ponta
cargo test --test e2e_pipeline
cargo test --test camara_pipeline
cargo test --test config_pipeline
cargo test --test dossie_pipeline
cargo test --test ingestion_pipeline

# Valida tipagem TypeScript e build do frontend
cd web && npm run check && npm run build
```

---

## 📄 Licença
Distribuído sob licença aberta para fortalecimento da transparência pública e controle social.
