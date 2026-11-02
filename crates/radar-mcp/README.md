# `radar-mcp` — Servidor MCP do Radar Cívico

O `radar-mcp` é o servidor **Model Context Protocol (MCP)** oficial do **Radar Cívico**, implementado em Rust sobre transporte `stdio` (JSON-RPC 2.0). Ele permite que clientes e agentes de IA (Claude Desktop, Claude Code, Cursor, entre outros) consultem a base local de auditoria cívica, dossiês de pessoas físicas e jurídicas, mandatos parlamentares e o grafo de relacionamentos societários/eleitorais de maneira **estritamente somente-leitura** e com proteção de janela de contexto.

---

## 1. Requisitos Prévios

Antes de iniciar o `radar-mcp`, certifique-se de que:

1. **API HTTP do Radar Cívico está em execução** (padrão em `http://127.0.0.1:28080`):
   ```bash
   docker compose up -d
   # Verifica se o serviço está saudável:
   curl -s http://127.0.0.1:28080/api/versao
   ```
2. **Rust Toolchain instalada** (`rustc` / `cargo` 1.75+ para compilação local do binário).
3. **Variáveis de Ambiente Suportadas**:
   - `RADAR_URL` *(opcional)*: URL base da API HTTP do Radar Cívico (default: `http://127.0.0.1:28080`).
   - `RADAR_API_KEY` *(opcional)*: Chave de autenticação enviada nos cabeçalhos `x-api-key` e `Authorization: Bearer <chave>` caso a instância local esteja protegida.

---

## 2. Compilação em Modo Release

Na raiz do workspace do repositório `Radar_Civico`, compile o binário otimizado:

```bash
cargo build --release --bin radar-mcp
```

O executável final será gerado em:

```text
target/release/radar-mcp
```

> **Dica:** Os logs operacionais são emitidos exclusivamente em `stderr`, mantendo `stdout` reservado para o protocolo JSON-RPC `stdio`.

---

## 3. Configuração no Claude Desktop (`claude_desktop_config.json`)

Adicione o servidor `radar-civico` ao arquivo `claude_desktop_config.json`:
- **Linux:** `~/.config/Claude/claude_desktop_config.json`
- **macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows:** `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "radar-civico": {
      "command": "/caminho/absoluto/para/Radar_Civico/target/release/radar-mcp",
      "args": [],
      "env": {
        "RADAR_URL": "http://127.0.0.1:28080",
        "RADAR_API_KEY": "sua-chave-opcional-aqui"
      }
    }
  }
}
```

---

## 4. Configuração no Claude Code e Cursor (via `stdio`)

### Claude Code (CLI)

Registre o servidor MCP diretamente pelo comando `claude mcp add` com escopo `stdio`:

```bash
claude mcp add radar-civico \
  --transport stdio \
  -e RADAR_URL=http://127.0.0.1:28080 \
  -e RADAR_API_KEY=sua-chave-opcional-aqui \
  -- /caminho/absoluto/para/Radar_Civico/target/release/radar-mcp
```

### Cursor (`.cursor/mcp.json` ou Configurações Globais MCP)

Crie ou edite `.cursor/mcp.json` na raiz do projeto (ou em `~/.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "radar-civico": {
      "type": "stdio",
      "command": "/caminho/absoluto/para/Radar_Civico/target/release/radar-mcp",
      "args": [],
      "env": {
        "RADAR_URL": "http://127.0.0.1:28080",
        "RADAR_API_KEY": "sua-chave-opcional-aqui"
      }
    }
  }
}
```

---

## 5. Ferramentas Disponíveis (Escopo Somente-Leitura)

Todas as ferramentas expostas pelo `radar-mcp` operam em regime **100% somente-leitura (`GET`)** sobre a API local do Radar Cívico, aplicando sanitização de parâmetros, mascaramento de dados sensíveis (`***.XXX.XXX-**` para CPFs) e truncamento de listas extensas para proteger o contexto da LLM.

| Ferramenta | Rota Consumida (`GET`) | Parâmetros Aceitos | Escopo e Comportamento |
| :--- | :--- | :--- | :--- |
| **`buscar`** | `/api/v1/busca?q={q}` | • `q` (`string`, **obrigatório**): termo, nome ou documento.<br>• `limite` (`integer`, opcional, default `10`, máx `25`). | **Somente-leitura.** Busca unificada de políticos, empresas e contratos. Retorna lista enxuta com `id`, `tipo`, `nome` e `relevancia`. |
| **`dossie_cnpj`** | `/api/v1/dossie/cnpj/{cnpj}` | • `cnpj` (`string`, **obrigatório**): CNPJ com ou sem pontuação (sanitizado automaticamente). | **Somente-leitura.** Retorna resumo cadastral/financeiro (CEAP, PNCP, doações), quadro societário (QSA truncado) e alertas heurísticos com nota metodológica. |
| **`listar_politicos`** | `/api/v1/politicos` | • `uf` (`string`, opcional, 2 letras).<br>• `partido` (`string`, opcional).<br>• `pagina` (`integer`, opcional, default `1`).<br>• `limite` (`integer`, opcional, default `20`, máx `50`). | **Somente-leitura.** Lista políticos cadastrados com filtros por estado e partido. Retorna `id`, `nome_eleitoral`, `cargo`, `partido` e `uf`. |
| **`detalhe_politico`** | `/api/v1/politicos/{id}` | • `id` (`integer` ou `string`, **obrigatório**): identificador interno ou sequencial do político. | **Somente-leitura.** Consulta o perfil completo do político, incluindo histórico de candidaturas e vínculos conhecidos, descartando metadados internos de UI. |
| **`dossie_cpf`** | `/api/v1/dossie/cpf/{cpf}` | • `cpf` (`string`, **obrigatório**): CPF com ou sem pontuação (sanitizado automaticamente). | **Somente-leitura.** Retorna sociedades empresariais, doações e alertas vinculados à pessoa física, aplicando mascaramento obrigatório no padrão `***.XXX.XXX-**`. |
| **`subgrafo`** | `/api/v1/grafo/{id}?grau=1` | • `id` (`string`, **obrigatório**): ex. `'cnpj:12345678000195'` ou `'cpf:12345678900'`.<br>• `limite_arestas` (`integer`, opcional, default `30`, máx `50`). | **Somente-leitura.** Obtém conexões imediatas (raio 1) no grafo de relacionamentos. Trunca nós e arestas no limite configurado e sinaliza `"truncado": true` quando aplicável. |
| **`listar_alertas`** | `/api/v1/auditoria/alertas` | • `tipo` (`string`, opcional): ex. `'doador_fornecedor'`, `'socio_fantasma'`.<br>• `severidade` (`string`, opcional): `'baixa'`, `'media'`, `'alta'`.<br>• `limite` (`integer`, opcional, default `15`, máx `50`). | **Somente-leitura.** Lista apontamentos heurísticos para triagem investigativa (`id`, `alerta_tipo`, `entidade_alvo`, `grau_severidade`, `resumo_fato`). Inclui aviso explícito de que não constituem comprovação de irregularidade ou condenação. |

---

## 6. Execução dos Testes

Para validar os testes unitários e a suíte de testes de integração `stdio` (com mocks HTTP determinísticos via `wiremock`):

```bash
cargo test -p radar-mcp
```
