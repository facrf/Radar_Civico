# Confiabilidade dos dados e operação em bases grandes

## Avaliação dos políticos

Listagem, dossiê e detalhe usam a mesma avaliação (`alertas-locais-v1`). O valor total de despesas não reduz a nota. O cálculo usa as severidades dos alertas persistidos, com a mesma fórmula da crate `auditor`.

`score_integridade` é `null` quando faltam dados, ainda não houve auditoria válida ou as fontes mudaram desde a última auditoria. `avaliacao_integridade` informa situação, cobertura, metodologia, data da auditoria e quantidades por severidade. Uma nota 100 significa ausência de alertas nas bases locais avaliadas. A cobertura local não equivale à cobertura nacional completa, nem comprova regularidade.

Novos dados e alterações dos parâmetros deixam a avaliação pendente. A auditoria só publica uma avaliação atual após confirmar sua transação. Os cartões apresentam indícios para conferência, e falhas de consulta têm mensagem e botão de nova tentativa.

## Autoridades e evidência

O arranque não insere o catálogo embutido. A ação explícita de catálogo continua disponível, com registros marcados como não verificados e sem CPF. Cargos desse catálogo não fornecem a evidência de posse exigida pela triangulação.

A migração preserva em `qualidade_dados_eventos` os documentos sem evidência que coincidam exatamente com nome e máscara do antigo catálogo, antes de retirá-los do cadastro operacional. Outros documentos não são alterados. Os cargos históricos são preservados, com indicação de ausência de conferência.

Para um catálogo com fontes conferidas, `RADAR_AUTORIDADES_JSON` pode indicar um arquivo JSON com objetos `NovaAutoridade`. Um objeto com `evidencia_verificada=true` exige `fonte_url` HTTP(S), `conferido_em` RFC3339 e `data_posse` válida. A conferência é responsabilidade de quem prepara o catálogo; o sistema valida os campos e não certifica automaticamente o conteúdo do ato. Catálogos manuais não substituem dados já verificados de um mesmo cargo.

## PNCP

A API de consultas recebe filtros por data de publicação e paginação. Ela não declara filtro `niFornecedor` no endpoint de contratos. Referência: [Swagger oficial do PNCP](https://pncp.gov.br/api/consulta/swagger-ui/index.html), especificação `/v1/contratos`.

Os fluxos individual, unificado e de importadores percorrem os doze meses do ano solicitado e todas as páginas de cada mês. Cada página tem até 500 registros, limite de resposta de 16 MiB e gravação própria. Um filtro de fornecedor, quando informado no importador, é aplicado localmente. Anos significam ano de publicação no PNCP, não necessariamente de assinatura.

Falhas de HTTP/gravação identificam mês e página. Páginas já confirmadas permanecem na base. O resultado distingue `CONCLUIDO`, `PARCIAL`, `ERRO` e cancelamento. Respostas 429/5xx admitem até três tentativas; páginas repetidas ou metadados inconsistentes impedem declarar coleta completa. A mensagem final apresenta páginas, meses completos, registros gravados e falhas. O gerenciador preserva a mensagem e o resultado parcial do importador.

## Retificações CEAP

A carga bulk, a API da Câmara e o upload manual reconhecem o identificador da fonte. Na Câmara, a chave combina código do documento, ressarcimento, parcela e lote; esses campos distinguem lançamentos de um mesmo documento. Uploads personalizados também podem declarar `ID_ORIGEM`.

Com chave estável, uma retificação mantém o ID interno e atualiza a despesa em vez de somar uma segunda linha. Valores anteriores e atuais ficam em `despesas_parlamentares_versoes`. Reimportar a mesma versão não grava outra revisão. Documentos oficiais diferentes podem ter o mesmo número, fornecedor, data e valor.

Para uma linha legada sem chave, a reconciliação automática exige uma única correspondência por casa, parlamentar, data, fornecedor e número do documento. Mais de uma candidata gera erro, evitando escolher arbitrariamente uma linha. Cargas sem identificador estável mantêm a deduplicação legada e não garantem reconciliação de retificações. Para esses arquivos, forneça `ID_ORIGEM` e confira os casos históricos sem correspondência inequívoca.

## Projeção incremental

A migração 29 instala uma fila transacional de alterações por entidade. Após a projeção inicial, as atualizações processam até 500 eventos por transação, preservando as arestas dos registros não alterados. Exclusões, mudanças de candidatura, dados CEAP e raízes QSA são acompanhados por triggers. Alterar apenas foto ou flag de auditoria não solicita reconstrução de relações.

O servidor drena a fila continuamente e tenta novamente após falhas. Cada lote e a remoção de seus eventos são atômicos. Uma falha mantém a projeção anterior daquele lote e os eventos pendentes. Uploads que já possuem uma transação externa preservam a atomicidade do arquivo.

O cache v2 íntegro é aproveitado na atualização; uma base sem projeção compatível ainda precisa de uma reconstrução completa inicial. `/health` indica que o processo HTTP responde. `/ready` verifica conexão, versão da projeção e ausência de pendências, retornando 503 enquanto a rede está desatualizada. A resposta inclui relações, eventos pendentes e duração da última atualização incremental.

## Backups e configuração

`POST /api/v1/config/backup` retorna HTTP 202 com `job_id` e nome do arquivo. O painel acompanha a tarefa e atualiza os downloads ao concluir. O snapshot e seu SHA-256 são produzidos fora das threads HTTP; arquivos parciais não aparecem na listagem. O diretório de backups acompanha o diretório do banco configurado. Backups concluídos não são apagados automaticamente.

A exportação direta antiga permanece disponível, com o snapshot também executado em uma tarefa bloqueante. As consultas principais de políticos e dossiês executam fora das threads HTTP. O detalhe devolve sua conexão antes de carregar o dossiê, evitando esgotar o pool com conexões aninhadas.

O backend de configurações delega PNCP, backups, integridade e identidade visual a módulos próprios. O frontend separa os contratos de configuração, o painel de backups e a avaliação de integridade.

## Validação e implantação

Node 22.17+ é necessário. `.nvmrc` e CI usam 22.17.0; o Dockerfile usa a família Node 22. Os testes comuns de CEAP e PNCP usam arquivos pequenos ou HTTP local, sem baixar dumps externos. O CI executa também os testes de polling e dos erros HTTP do frontend.

```bash
cargo check --workspace --all-targets --locked
cargo test --all --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
npm --prefix web run check
npm --prefix web test
npm --prefix web run build
docker compose up --build -d
docker compose ps
docker logs --tail 100 radar_civico
curl -f http://127.0.0.1:28080/ready
```

Antes da atualização de uma base existente, preserve um snapshot consistente do SQLite e mantenha os arquivos de conclusão de tarefas no volume de dados. Migrações são versionadas: não substitua uma base migrada por um binário antigo sem restaurar o snapshot correspondente.
