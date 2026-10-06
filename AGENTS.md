# AGENTES.MD - Protocolo Operacional do Radar Cívico

Este documento define os papéis, escopos e diretrizes de execução para agentes de IA operando no repositório `radar-civico`. Cada agente deve respeitar estritamente o seu diretório de atuação e os critérios de validação antes de commitar qualquer alteração.

---

## 1. Regras Gerais de Conduta (Modo Autônomo / YOLO)

1. **Compilação como Barreira:** Todo agente trabalhando em Rust deve executar `cargo check` e `cargo test` antes de considerar uma tarefa concluída. Nenhuma alteração que quebre a compilação deve persistir.
2. **Proibição de Stubs:** É expressamente vedado o uso de `todo!()`, `unimplemented!()` ou funções vazias sem tratamento de erro.
3. **Escopo Fechado:** Nenhum agente deve modificar arquivos fora de sua pasta designada sem antes atualizar a interface pública da crate correspondente.
4. **Tratamento de Erros:** Erros internos das crates utilizam `thiserror`. Apenas a camada final de aplicação (`server` ou binário de CLI) utiliza `anyhow`.

---

## 2. Definição dos Agentes

### Agente 1: Storage & Database (Crate `crates/storage`)
* **Missão:** Gerenciar o ciclo de vida do SQLite, otimizações de baixo nível e integridade relacional.
* **Escopo:** `crates/storage/**`
* **Responsabilidades:**
  * Configurar conexões com aplicação obrigatória dos PRAGMAs (`WAL`, `synchronous = NORMAL`, `cache_size = -64000`, `temp_store = MEMORY`, `foreign_keys = ON`).
  * Implementar migrações SQL versionadas para as tabelas `politicos`, `candidaturas`, `receitas_campanha`, `despesas_parlamentares`, `registros_profissionais`, `nos_rede` e `conexoes_rede`.
  * Garantir inserções em lote (*batch insert*) dentro de transações explícitas para manter alto throughput.
* **Critério de Sucesso:** Testes unitários com banco em memória passando e validação de schema sem erros.

### Agente 2: Ingestion & Streaming (Crate `crates/ingestion`)
* **Missão:** Processar dumps massivos e APIs públicas com consumo eficiente de memória.
* **Escopo:** `crates/ingestion/**`
* **Responsabilidades:**
  * Ler arquivos CSV compactados em blocos utilizando streams assíncronos (`csv-async`).
  * Normalizar documentos fiscais (limpeza de caracteres não numéricos em CNPJs e CPFs mascarados).
  * Tratar divergências de encoding (ISO-8859-1 para UTF-8 típico em arquivos antigos do TSE).
  * Encaminhar registros em lotes para a camada de `storage`.
* **Critério de Sucesso:** Processamento de buffers mockados sem alocação descontrolada de memória RAM.

### Agente 3: Auditor & Heurísticas (Crate `crates/auditor`)
* **Missão:** Implementar regras determinísticas e cruzamentos analíticos para identificação de irregularidades.
* **Escopo:** `crates/auditor/**`
* **Responsabilidades:**
  * **Regra de Combustível:** Identificar notas da CEAP com volume faturado superior ao limite físico do tanque de veículos leves (> 80 litros).
  * **Conflito de Interesses (Art. 28 da Lei 8.906/94):** Identificar nomeados em secretarias municipais/estaduais que constem com inscrição "Regular" no Cadastro Nacional dos Advogados (OAB).
  * **Inexigibilidade Suspeita:** Sinalizar contratos públicos firmados com escritórios ou empresas cujos sócios constem como doadores de campanha em janela inferior a 180 dias da posse.
  * **Fornecedor Hub:** Calcular concentração de repasses partidários e de campanha canalizados para pessoas jurídicas comuns.
* **Critério de Sucesso:** Testes determinísticos cobrindo casos reais de alerta e casos de controle (sem falso positivo).

### Agente 4: Web & Visualização (Pasta `web/`)
* **Missão:** Interface do usuário, dossiê dos alvos e exibição interativa de redes.
* **Escopo:** `web/**`
* **Responsabilidades:**
  * Desenvolver interface com layout limpo e responsivo.
  * Construir a visualização de grafos (Cytoscape.js ou Apache ECharts) consumindo os nós (`nos_rede`) e arestas (`conexoes_rede`).
  * Formatar valores monetários e datas no padrão brasileiro (BRL / DD/MM/AAAA).
  * Exibir alertas emitidos pelo auditor em cartões visuais com indicação clara da fonte pública primária.
* **Critério de Sucesso:** Build do frontend concluído sem avisos de tipagem ou dependências ausentes.

---

## 3. Fluxo de Integração Git

Para manter o repositório estável durante operações contínuas:
1. Concluir a unidade funcional designada.
2. Executar suíte de validação: `cargo test --all`.
3. Criar commit atômico seguindo o padrão:
   * `feat(storage): adiciona migration para registros da oab`
   * `feat(auditor): implementa heuristica de limite de combustivel`
   * `feat(ingestion): adiciona parser streaming para receitas tse`
   
   
Critérios obrigatórios de execução e entrega:
1. Faça as modificações necessárias nos arquivos de código e configuração.
2. REQUISITO DE RUNTIME: Não encerre a tarefa apenas editando os arquivos. Como a aplicação roda conteinerizada, você DEVE executar o rebuild da imagem Docker e recriar/reiniciar o contêiner (ex.: `docker compose up --build -d` ou comando equivalente do projeto).
3. Verifique os logs do contêiner (`docker logs`) e o status da execução para garantir que a aplicação subiu sem quebras.
4. Apenas dê a tarefa por concluída após o serviço estar rodando com a nova imagem ativa.