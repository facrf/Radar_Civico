# Correções de confiabilidade — 16/11/2026

A revisão identificou sete problemas no cálculo, na cobertura e na operação das auditorias. Este documento registra as correções, as mudanças de contrato e como validá-las. Os alertas são indícios para conferência das fontes; não constituem comprovação de irregularidade.

## Achados e correções

| Achado | Comportamento corrigido | Regressão |
| --- | --- | --- |
| Doação circular somava depois de um cruzamento muitos para muitos | Despesas e receitas são agregadas separadamente em centavos, antes do cruzamento por político, ano eleitoral e documento. A ligação por nome exige identidade única; documentos completos inválidos são excluídos. | Duas despesas de R$ 3.000 e três doações de R$ 400 continuam totalizando R$ 6.000 e R$ 1.200. Não passam no piso de R$ 10.000 de CEAP. |
| Erros SQL e de gravação eram descartados | Consultas, leitura de linhas e gravação de alertas propagam erros. A transação da auditoria é revertida; a conclusão só é publicada após commit. | Trigger que recusa a gravação de fracionamento impede a conclusão; nova tentativa sem a falha grava o alerta. |
| Investigação OAB confundia indisponibilidade, homônimos e cargo vigente | Estados das fontes e avisos fazem parte da resposta. Trechos textuais não confirmam nomeação e nenhuma data é inventada. Alerta exige identidade documental e cargo verificado. | Falha das fontes, homônimo com CPF diferente, cargo encerrado e inscrição antiga não geram confirmação. O caso positivo usa documentos completos e ato verificado. |
| Senha vazia desativava chave de API preenchida | Cada credencial é normalizada e validada independentemente. O Compose repassa ambas as variáveis de ambiente. | Senha ausente, vazia ou com espaços não desativa uma chave configurada; senha e chave distintas funcionam. |
| Cortes silenciosos na cobertura | Foram removidos os cortes de 10 mil despesas, 150 políticos, 50 sócios e 300 pagamentos. O fracionamento percorre as despesas ordenadas e retém a janela corrente de um fornecedor. Datas inválidas geram erro explícito. | Casos após as primeiras 10 mil despesas e após o político 150 são detectados; o fluxo ordenado equivale ao algoritmo em lote. |
| Auditoria de hubs materializava a rede completa | A nova API de `graph` visita empresas e agrega pagamentos no SQLite. Não cria um `petgraph` nacional nem executa Brandes durante a auditoria. Centralidade é `null`, e grupos sem identificação não são considerados evidência de concentração política. | Equivalência financeira com um grafo pequeno conhecido, preservação de centavos, controle de grupo desconhecido e propagação de falha ao salvar. |
| Defaults divergiam do protocolo | Backend e interface usam 80 litros e concentração de 70%. Parâmetros explicitamente persistidos continuam respeitados. | A API de configurações retorna os defaults e preserva valores personalizados. |

Os totais de doação circular descrevem os registros disponíveis nas bases locais; a coincidência não demonstra nexo causal. A concentração considera relações financeiras positivas (`DESPESA`, `DESPESA_CAMPANHA`, `CEAP`, `CONTRATO`). Arestas societárias não entram no faturamento. Quando o nó projetado informa somente `politico_id`, o grupo usa o partido da candidatura do mesmo ano, desde que ele seja único; partidos conflitantes não são resolvidos por escolha arbitrária. A nova consulta de concentração limita a memória da aplicação à empresa corrente; ela não é um benchmark de tempo para a base nacional. A auditoria completa continua sendo uma operação de escrita transacional e pode disputar o escritor SQLite com importações.

## Contrato da investigação OAB

`GET /api/v1/investigar/nomeacao/{identificador}` mantém os campos de consulta e acrescenta `fontes` e `avisos`. A API informa por fonte `DISPONIVEL`, `CACHE_LOCAL`, `BASE_LOCAL`, `CACHE_APOS_FALHA` ou `INDISPONIVEL`.

Os estados da análise são:

- `ALERTA_GERADO`: existe um indício com documentação suficiente; consultar também o estado das fontes.
- `SEM_INDICIO_NAS_FONTES_CONSULTADAS`: o registro confirmado está licenciado/cancelado, sem falha das fontes consultadas.
- `EVIDENCIA_INSUFICIENTE`: os dados disponíveis não permitem confirmar o vínculo e a vigência.
- `ANALISE_PARCIAL`: alguma fonte falhou ou foi necessário recorrer a cache após falha.
- `FONTE_INDISPONIVEL`: nenhuma das consultas externas necessárias foi bem-sucedida e não havia cache utilizável.

`REGULAR_SEM_ALERTA` foi retirado. Clientes devem tratar explicitamente os novos estados, sem converter ausência de alerta em comprovação de regularidade.

A confirmação exige CPF completo matematicamente válido e igual no doador, no político associado ao cargo e no registro profissional local; inscrição com carimbo local de consulta de até 30 dias; cargo com `evidencia_verificada=1`, URL HTTP(S), conferência RFC3339, posse anterior ou igual à data atual e sem exoneração já ocorrida. O carimbo local não certifica automaticamente a atualidade da fonte primária. A conferência do ato e do cadastro é responsabilidade de quem prepara a fonte.

Busca remota por nome não recebe artificialmente o CPF pesquisado. Registros mascarados e resultados por nome permanecem evidência possível. O upload OAB exige situação cadastral explícita; situação ausente não é preenchida como `REGULAR`. O CNA pode exigir interação humana; o caminho operacional é importar dados oficiais conferidos. A API libera as conexões SQLite durante as consultas HTTP.

## Atualização de bases existentes

A migração 35 arquiva os alertas afetados em `qualidade_dados_eventos`, incluindo o conteúdo anterior, e os retira do conjunto de alertas ativos. Fontes financeiras, cadastros e parâmetros personalizados são preservados. A metodologia passa a `alertas-locais-v2`, com avaliação pendente até uma auditoria completa bem-sucedida.

Uma execução bem-sucedida também retira alertas dessas regras que deixaram de satisfazer os dados atuais. Erros impedem essa substituição. O histórico arquivado pode ser consultado com:

```sql
SELECT registro_id, valor_anterior, motivo, criado_em
FROM qualidade_dados_eventos
WHERE entidade = 'alertas_auditoria'
ORDER BY id DESC;
```

A reauditoria da base é uma ação explícita em Configurações/Alertas ou pelo endpoint `POST /api/v1/auditoria/sincronizar`. Em bases grandes, reserve a janela de operação; o healthcheck não depende de concluir essa auditoria. Não executar a auditoria significa manter a avaliação pendente, sem apresentar uma nota atual baseada no motor anterior.

## Validação e operação

```bash
cargo check --all --locked
cargo test --all --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
npm --prefix web run check
npm --prefix web test
npm --prefix web run build
docker compose config --quiet
make docker-up
docker compose ps
docker logs --tail 40 radar_civico
curl -fsS http://127.0.0.1:28080/health
curl -fsS http://127.0.0.1:28080/ready
curl -fsS http://127.0.0.1:28080/api/v1/config/versao
```

`make docker-up` atualiza `version.json` a partir do Git antes do rebuild, evitando publicar metadados antigos. Builds automatizados podem fornecer `APP_VERSION`, `APP_COMMIT` e `APP_COUNT`. O arquivo de versão local é gerado e não é versionado.

As credenciais `RADAR_ADMIN_PASSWORD` e `RADAR_API_KEY` podem ser fornecidas no ambiente ou no `.env` lido pelo Compose. Ambas vazias mantêm o modo local livre. Não inclua credenciais reais no repositório.
