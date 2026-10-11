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

Validação local em 10/10/2026 (America/Sao_Paulo):

- `cargo check --workspace --locked`: aprovado.
- `cargo test --all --locked`: 172 testes aprovados.
- `npm run check`: zero erros e zero avisos; `npm run build`: aprovado.
- YAML do workflow carregado com sucesso. O workflow remoto ainda depende do envio do commit ao GitHub.
- Imagem reconstruída com BuildKit e cache. Contêiner recriado, estado `running/healthy`, `/health` retornando `OK` e logs de inicialização sem falhas.
- Migração 21 confirmada no banco real. Projeção monetária conferida como `integer`; regras existentes de combustível preservadas.
- Endpoints de configurações, importadores e progresso retornaram HTTP 200. Jobs e raízes de grafo inexistentes retornaram HTTP 404.
- Dossiê real consultado com limite de uma nota; total monetário conferido diretamente no SQLite. A regressão de paginação cobre 201 notas e 61 contratos em banco de teste.
- Imagem ativa: `sha256:8f6901d4b6f7451784d52fffcb9b2ec52dfc181cc8f455be4282abba3f3a23e2`.
