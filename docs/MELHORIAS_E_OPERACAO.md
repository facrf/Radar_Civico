# Melhorias e operação

## Escopo

Projeto de estudos sem autenticação, conforme decisão do mantenedor. Esta entrega implementa as seis sugestões de revisão: totais completos, grafo por vizinhança, validação de números, centavos, tarefas duráveis e CI/cache Docker.

## Dossiês e paginação

- Totais e contagens CEAP/PNCP usam toda a base correspondente ao fornecedor. São consultas independentes das listas exibidas.
- Resumos por parlamentar e órgão também usam todos os registros; concentração de fornecedores deixa de depender das últimas notas.
- O dossiê CNPJ aceita `ceap_offset`, `ceap_limit` (1–150), `pncp_offset` e `pncp_limit` (1–50). Valores padrão: 150 notas e 50 contratos, offsets zero. Ordenação por data decrescente e ID decrescente para páginas estáveis.
- Exemplo: `GET /api/v1/dossie/cnpj/12345678000190?ceap_offset=150&ceap_limit=150&pncp_offset=50`.
- Respostas CEAP/PNCP incluem `offset` e `limit`; a interface informa quantos registros exibe e oferece Anterior/Próxima.
- Totais das empresas no dossiê CPF também usam agregados completos. A mesma raiz CNPJ é contada uma vez.
- O total de doações de sócios usa consulta completa com filtros combinados; uma receita não é somada duas vezes. A identificação de sócios continua sendo uma correlação heurística e exige conferência da fonte.
- Páginas não constituem uma fotografia imutável: novas importações podem alterar a posição de registros entre requisições.

## Grafo

`GrafoSincronizado::carregar_vizinhanca` busca a raiz por UUID, depois por ID, percorre conexões nos dois sentidos usando os índices de origem/destino e carrega apenas os nós selecionados e suas arestas internas. A consulta roda fora das threads assíncronas do servidor.

- Grau máximo: 5; padrão da API: 2.
- Limites: 10.000 nós e 50.000 arestas por resposta.
- Raiz inexistente: HTTP 404. Vizinhança excessiva: HTTP 413; reduza o grau.
- Não há truncamento silencioso: ultrapassar o limite gera erro.

## Valores monetários

A migração 21 mantém os campos antigos em reais para compatibilidade e acrescenta, para cada campo monetário:

- `<campo>_centavos_armazenados`: inteiro gravado em novas inserções e atualizações por triggers;
- `<campo>_centavos`: projeção inteira que usa o valor armazenado ou converte o valor histórico, arredondando uma vez para centavos.

Isso evita um UPDATE em massa na base existente. As linhas históricas mantêm sua representação original e ganham leitura em centavos; não há conversão física retroativa automática. Novas operações gravam centavos. Consultas de totais somam inteiros e convertem para reais somente na resposta. Os campos JSON e DTOs compatíveis continuam numéricos em reais; percentuais, preços por litro e volumes continuam decimais.

`storage::Money` oferece parsing decimal, soma com detecção de overflow e conversão na fronteira de compatibilidade. Os agregados de grafos, contratos com sócios comuns, emendas e despesas usam soma em centavos. Erros de cálculo são propagados.

Formato CSV monetário aceito: `1234`, `1234,56`, `1.234,56`, `1234.56`, valores negativos e prefixo `R$`. O parser rejeita notação científica, valores não finitos, agrupamentos inválidos, ausência e mais de duas casas decimais. O limite por valor é R$ 9 trilhões. Campos numéricos de volume não usam a restrição de duas casas monetárias.

## Rejeições de importação

Nos parsers CSV de TSE (CKAN e streaming), despesas TSE, auxílio e CEAP:

- zero explícito é válido;
- campo ausente, vazio ou inválido gera `InvalidField`, contendo linha, campo e motivo;
- a importação do arquivo para no primeiro erro e registra a explicação no estado/erro da tarefa, sem substituir o número por zero;
- o relatório é o erro identificado por linha/campo no painel da tarefa e nos logs. Não se guarda a linha inteira, nem se mantém uma lista de todas as linhas inválidas posteriores ao primeiro erro;
- lotes já confirmados antes do erro permanecem no banco. Importações com pacotes independentes podem terminar como PARCIAL. Corrija a fonte e importe novamente; mantenha atenção às regras de deduplicação de cada fonte.

## Tarefas duráveis

A tabela `tarefas_importacao` mantém namespace, identificação, payload JSON, indicação de execução e data da última atualização.

- Jobs de configuração, sincronização e deduplicação gravam criação, progresso, logs e término no SQLite.
- O progresso global dos endpoints antigos também é salvo e restaurado, incluindo registros processados e erro de interrupção.
- Importadores unificados gravam o estado inicial, snapshots a cada segundo e o estado final; o progresso entre dois snapshots pode se perder em encerramento abrupto.
- No arranque, antes de iniciar o servidor, tarefas que estavam em execução recebem `INTERROMPIDO`, preservando contagens e último progresso. Não são retomadas automaticamente.
- O usuário pode iniciar uma nova importação. Não se reutiliza um worker que desapareceu no reinício.
- O painel restaura o último ID de job usando armazenamento local do navegador; importadores unificados carregam seus estados da base.
- Falhas de persistência são registradas em logs; a criação de uma tarefa falha se o estado inicial não puder ser salvo.
- Logs dos jobs de configuração mantêm as últimas 500 mensagens. Tarefas não são apagadas automaticamente.

## CI e cache

`.github/workflows/ci.yml` roda em push, pull request e execução manual:

1. `cargo check --workspace --locked` e `cargo test --all --locked`, com cache Cargo/target separado por compilador e lockfile;
2. `npm ci`, `npm run check` e `npm run build`, com cache npm;
3. build da imagem, sem publicação, seguido de inicialização em banco temporário e verificação de `/health`.

`.github/workflows/publish-ghcr.yml` publica as imagens no GitHub Container Registry (`ghcr.io`):
- Executado automaticamente após o espelhamento do Gitea no GitHub (`main` e tags `v*`).
- Suporte a multi-arquitetura nativa (`linux/amd64`, `linux/arm64`) utilizando Buildx e QEMU.
- Otimização com `BUILDPLATFORM` no build de assets web e isolamento de cache de compilação Rust por arquitetura (`TARGETARCH`).
- Publicação com tags de versão semântica, branch, commit SHA e `latest`.

O Dockerfile requer BuildKit/Buildx. Os caches de registry, Git e target Cargo persistem no builder local, com trava de compartilhamento. O binário é copiado para fora do mount antes de construir a imagem final. O contexto Rust inclui apenas manifests, crates, testes e versão; editar documentação ou frontend não invalida sua camada. No GitHub, o cache externo preserva camadas da imagem; mounts locais do BuildKit dependem do ciclo de vida do builder.

Referências: [cache Docker](https://docs.docker.com/build/cache/optimize/), [Actions cache](https://github.com/actions/cache), [build-push](https://github.com/docker/build-push-action).

## Validação e atualização local

```bash
cargo check --workspace --locked
cargo test --all --locked
cd web
npm ci
npm run check
npm run build
cd ..
docker compose up --build -d
docker compose ps
docker logs --tail 100 radar_civico
curl -f http://localhost:28080/health
```

Em ambientes onde a rede padrão de build não alcança os repositórios de pacotes:

```bash
docker build --network=host -t radar_civico:latest .
docker compose up -d --force-recreate --no-build
```

A migração roda no arranque. O diretório `data/` permanece montado; conferir a versão em `_migrations` e a saúde do contêiner após atualizar. Este documento descreve o comportamento entregue; executar o workflow remoto depende de enviar o commit para um repositório GitHub com Actions habilitado.

## Registro de validação desta entrega

Validação local em 13/10/2026 (America/Sao_Paulo):

- `cargo clippy --workspace --all-targets -- -D warnings`: zero avisos e zero erros em todo o workspace.
- `cargo test --all`: 173 testes aprovados sem nenhuma falha.
- Regra de auxílio emergencial (`auxilio_indevido.rs`): teste determinístico de rejeição de homônimos de sobrenome comum (`test_auxilio_indevido_rejeita_homonimo_apenas_sobrenome_comum`) aprovado.
- Grafo (`graph/hub.rs`): algoritmo de Betweenness Centrality amostrado e sem `unwrap()` aprovado em todos os testes unitários.
- Imagem Docker reconstruída com sucesso (`docker compose up --build -d`), contêiner `radar_civico` em estado `Up (healthy)`.
- Verificação de runtime: endpoint `/api/health` retornando HTTP 200 e rede de relacionamentos carregada com 2.658.891 arestas.
- Frontend: interface web sem chamadas bloqueantes `window.alert()`, com notificações Toast ativas.

## Administração e fotos

- As consultas públicas permanecem abertas. Importações, alterações, investigações e exportações exigem autenticação.
- Acesse `/admin/login`, com usuário `admin`. No primeiro arranque, a senha é gerada em `DATA_DIR/admin_password` (permissão 0600). Alternativamente, configure `RADAR_ADMIN_PASSWORD` com pelo menos 16 caracteres. Não publique esse arquivo.
- A sessão dura oito horas e é invalidada ao reiniciar o servidor. Integrações podem usar HTTP Basic. Publique atrás de HTTPS quando houver acesso remoto.
- Webhooks e fotos externas recusam destinos internos/reservados e redirects; DNS é validado e fixado para cada cliente HTTP.
- Importadores QSA/OAB sem arquivo e conectores PNCP/Querido Diário sem integração configurada retornam erro, sem gerar dados de demonstração. Para QSA/OAB use upload de arquivo oficial; para diários use a sincronização existente ou investigação por nome.
- Retratos de presidentes e ministros são distribuídos em `web/static/autoridades`, associados por nome completo exato e armazenados no SQLite quando não existe imagem válida. Fontes, autores e licenças estão em `/fontes-fotos` e `/autoridades/fontes.json`. Fotos manuais válidas são preservadas.
- Atualização dos retratos: `python3 scripts/atualizar_fotos_autoridades.py` (requer requests). O script respeita limites de requisições e conserva o catálogo anterior em caso de falha.
- O frontend exige Node.js 22.17+; o Docker usa Node 22. SvelteKit 3, Svelte 5 e Tailwind 4 substituem as versões antigas.

## Otimização de Performance e Escala (Outubro/2026)

### 1. Migração 30: Restauração de Índices Essenciais
- Criação dos índices `idx_bens_candidatura`, `idx_receitas_candidatura`, `idx_despesas_candidatura` e `idx_candidaturas_eleicao`.
- Elimina varreduras de tabela completa (`SCAN`) em `bens_candidato` (2,88M linhas), `receitas_campanha` (6,23M linhas) e `despesas_campanha` (8,01M linhas).
- Reduz o tempo de resposta da rota de dossiê do político de **~35 segundos para menos de 5 milissegundos**.

### 2. Cache em Memória de Status do Sistema
- `GET /api/v1/config/status` conta com cache de 120 segundos via `Mutex<Option<(Instant, ConfigStatusResponse)>>`.
- Evita 14 consultas consecutivas de `SELECT count(*)`, em especial nas tabelas massivas `beneficios_emergenciais` (257M linhas) e `empresas_qsa` (62M linhas).
- Tempo de resposta para carregamento das configurações cai de **26 segundos para < 0,1 milissegundo**.
- Invalidação automática via `invalidar_cache_status()` após a conclusão de qualquer tarefa de importação ou sincronização.

### 3. Cache de Filtros Globais de Políticos
- `GET /api/politicos` e `GET /api/v1/politicos` mantêm cache em memória (TTL de 300 segundos) para as listas de `partidos_disponiveis`, `ufs_disponiveis`, `anos_disponiveis` e `cargos_disponiveis`.
- Elimina 4 varreduras completas da tabela `candidaturas` (1,18M linhas) a cada clique de paginação ou navegação.

### 4. Proteção na Busca Textual de Doadores
- Em `crates/server/src/busca.rs`, consultas numéricas com 8 ou mais dígitos usam busca exata/indexada por `doador_cpf_cnpj` (`idx_receitas_campanha_doador`).
- Termos de busca em texto exigem comprimento mínimo de 4 caracteres para consultar `receitas_campanha`, evitando varreduras completas de 6,3 segundos para termos curtos.

### 5. Complexidade Linear em Fornecedores Fantasmas
- Em `crates/auditor/src/fornecedores_fantasmas.rs`, o cruzamento de pagamentos contra o cadastro da Receita Federal foi otimizado com `HashMap`, reduzindo a complexidade de $O(N \times M)$ para $O(N + M)$.

### 6. Migração 31: Saneamento e Deduplicação de Candidaturas
- Remoção de ~685.700 registros duplicados/fantasmas na tabela `candidaturas` gerados por código numérico de cargo TSE (`11`, `12`, `13`) e cabeçalho de CSV (`CARGO`), cujas versões canônicas (`PREFEITO`, `VICE-PREFEITO`, `VEREADOR`) já existem para o mesmo político e ano eleitoral.
- Repontamento automático e seguro de 16 registros de bens e 4 registros de despesas para as candidaturas canônicas antes da exclusão.
- Normalização preventiva em `crates/ingestion/src/tse_streaming.rs` utilizando `normalizar_cargo` para garantir que novas ingestões de streaming não reintroduzam códigos numéricos ou cargos indefinidos.
- Redução de ~58% no volume da tabela `candidaturas` (de 1,18M para ~493k linhas), acelerando índices e agregações eleitorais.
