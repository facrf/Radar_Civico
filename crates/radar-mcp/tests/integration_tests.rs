use std::io::{BufRead, BufReader, Write};
use std::process::{Command as StdCommand, Stdio};

use assert_cmd::Command;
use serde_json::{Value, json};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Executa o binário `radar-mcp` via `assert_cmd` alimentando `stdin` com linhas JSON-RPC
/// dentro de `spawn_blocking` para não bloquear o runtime assíncrono do `wiremock`.
async fn run_mcp_batch(radar_url: String, api_key: Option<String>, lines: Vec<Value>) -> Vec<Value> {
    tokio::task::spawn_blocking(move || {
        let mut input = String::new();
        for line in lines {
            input.push_str(&serde_json::to_string(&line).expect("deve serializar JSON-RPC"));
            input.push('\n');
        }

        let mut cmd = Command::cargo_bin("radar-mcp").expect("binário radar-mcp deve existir");
        cmd.env("RADAR_URL", &radar_url);
        if let Some(key) = api_key {
            cmd.env("RADAR_API_KEY", key);
        } else {
            cmd.env_remove("RADAR_API_KEY");
        }

        let output = cmd
            .write_stdin(input)
            .output()
            .expect("deve executar o processo radar-mcp");

        assert!(
            output.status.success(),
            "radar-mcp encerrou com falha (status: {:?}). stderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout_str =
            String::from_utf8(output.stdout).expect("stdout do radar-mcp deve ser UTF-8 válido");

        stdout_str
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                serde_json::from_str::<Value>(l).unwrap_or_else(|e| {
                    panic!("Linha inválida no stdout JSON-RPC: '{l}' ({e})")
                })
            })
            .collect()
    })
    .await
    .expect("join de spawn_blocking deve concluir")
}

fn find_response_by_id(responses: &[Value], id: i64) -> &Value {
    responses
        .iter()
        .find(|r| r.get("id").and_then(Value::as_i64) == Some(id))
        .unwrap_or_else(|| panic!("Resposta JSON-RPC com id={id} não encontrada em: {responses:?}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_stdio_initialize_e_tools_list_7_ferramentas_com_schemas() {
    let mock_server = MockServer::start().await;

    let requests = vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "integration-test-client",
                    "version": "1.0.0"
                }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    ];

    let responses = run_mcp_batch(mock_server.uri(), None, requests).await;
    assert_eq!(responses.len(), 2, "Esperadas respostas para id=1 e id=2");

    // 1. Valida handshake `initialize`
    let init_resp = find_response_by_id(&responses, 1);
    assert_eq!(init_resp["jsonrpc"], "2.0");
    assert_eq!(init_resp["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "radar-mcp");
    assert!(init_resp["result"]["capabilities"]["tools"].is_object());
    assert_eq!(init_resp["result"]["capabilities"]["prompts"], json!({}));

    // 2. Valida `tools/list` contendo exatamente as 7 ferramentas e seus schemas
    let list_resp = find_response_by_id(&responses, 2);
    let tools = list_resp["result"]["tools"]
        .as_array()
        .expect("result.tools deve ser um array");

    assert_eq!(
        tools.len(),
        7,
        "O servidor MCP deve listar todas as 7 ferramentas: {tools:?}"
    );

    let expected_tools = [
        ("buscar", vec!["q"]),
        ("dossie_cnpj", vec!["cnpj"]),
        ("listar_politicos", vec![]),
        ("detalhe_politico", vec!["id"]),
        ("dossie_cpf", vec!["cpf"]),
        ("subgrafo", vec!["id"]),
        ("listar_alertas", vec![]),
    ];

    for (expected_name, required_fields) in expected_tools {
        let tool = tools
            .iter()
            .find(|t| t["name"].as_str() == Some(expected_name))
            .unwrap_or_else(|| panic!("Ferramenta '{expected_name}' ausente em tools/list"));

        let description = tool["description"]
            .as_str()
            .expect("descrição deve estar presente");
        assert!(!description.is_empty());

        if expected_name == "listar_alertas" {
            assert!(
                description.contains("ATENÇÃO: Os alertas são apontamentos heurísticos"),
                "listar_alertas deve conter o aviso explícito de natureza heurística"
            );
        }

        let schema = &tool["inputSchema"];
        assert_eq!(
            schema["type"], "object",
            "inputSchema de '{expected_name}' deve ter type='object'"
        );
        assert!(
            schema["properties"].is_object(),
            "inputSchema de '{expected_name}' deve ter 'properties'"
        );

        for req_field in required_fields {
            let req_arr = schema["required"]
                .as_array()
                .unwrap_or_else(|| panic!("'{expected_name}' deve possuir array 'required'"));
            assert!(
                req_arr.iter().any(|v| v.as_str() == Some(req_field)),
                "Campo obrigatório '{req_field}' ausente no schema de '{expected_name}'"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_stdio_tools_call_dossie_cnpj_formata_resposta_corretamente() {
    let mock_server = MockServer::start().await;

    let mock_payload = json!({
        "avisos_identidade": [],
        "cnpj": "00000000000191",
        "cnpj_formatado": "00.000.000/0001-91",
        "razao_social": "BANCO DO BRASIL SA",
        "situacao_cadastral": "ATIVA",
        "data_abertura": "1966-08-01",
        "score_risco": "MEDIO",
        "total_alertas": 1,
        "alertas": [
            {
                "tipo": "FORNECEDOR_HUB",
                "severidade": "MEDIA",
                "titulo": "Concentração de repasses",
                "descricao": "Fornecedor recebeu pagamentos de múltiplos gabinetes na mesma competência.",
                "valor_envolvido": 84500.0,
                "fonte": "CEAP / Câmara dos Deputados"
            }
        ],
        "qsa": {
            "total_socios": 1,
            "socios": [
                {
                    "nome": "DIRETORIA EXECUTIVA TESTE",
                    "documento_mascarado": "***.111.222-**",
                    "qualificacao": "Diretor",
                    "tipo": "PF",
                    "outras_empresas": [
                        { "cnpj": "11111111000111", "razao_social": "SUBSIDIARIA SA" }
                    ]
                }
            ]
        },
        "ceap": {
            "total_faturado": 12000.50,
            "total_notas": 4
        },
        "pncp": {
            "total_contratado": 950000.0,
            "total_contratos": 2
        },
        "tse": {
            "total_doacoes_socios": 0.0
        }
    });

    Mock::given(method("GET"))
        .and(path("/api/v1/dossie/cnpj/00000000000191"))
        .and(header("x-api-key", "chave-integracao-mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_payload))
        .expect(1)
        .mount(&mock_server)
        .await;

    let requests = vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test-dossie-cnpj", "version": "0.1.0" }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "dossie_cnpj",
                "arguments": {
                    "cnpj": "00.000.000/0001-91"
                }
            }
        }),
    ];

    let responses = run_mcp_batch(
        mock_server.uri(),
        Some("chave-integracao-mcp".to_string()),
        requests,
    )
    .await;

    let call_resp = find_response_by_id(&responses, 2);
    assert_eq!(call_resp["result"]["isError"], false);

    let text = call_resp["result"]["content"][0]["text"]
        .as_str()
        .expect("deve retornar bloco de texto com JSON formatado");
    let formatted: Value =
        serde_json::from_str(text).expect("conteúdo retornado por dossie_cnpj deve ser JSON válido");

    assert_eq!(
        formatted["resumo_cadastral"]["cnpj"],
        "00000000000191"
    );
    assert_eq!(
        formatted["resumo_cadastral"]["cnpj_formatado"],
        "00.000.000/0001-91"
    );
    assert_eq!(
        formatted["resumo_cadastral"]["razao_social"],
        "BANCO DO BRASIL SA"
    );
    assert_eq!(formatted["resumo_cadastral"]["situacao_cadastral"], "ATIVA");
    assert_eq!(formatted["resumo_cadastral"]["score_risco"], "MEDIO");
    assert_eq!(
        formatted["resumo_cadastral"]["total_faturado_ceap"],
        12000.5
    );
    assert_eq!(
        formatted["resumo_cadastral"]["total_contratado_pncp"],
        950000.0
    );
    assert_eq!(formatted["total_socios"], 1);
    assert_eq!(formatted["socios_truncados"], false);
    assert_eq!(
        formatted["socios"][0]["nome"],
        "DIRETORIA EXECUTIVA TESTE"
    );
    assert_eq!(formatted["socios"][0]["total_outras_empresas"], 1);
    assert_eq!(formatted["total_alertas"], 1);
    assert_eq!(formatted["alertas"][0]["tipo"], "FORNECEDOR_HUB");
    assert!(
        formatted["nota_metodologica"]
            .as_str()
            .unwrap_or_default()
            .contains("heurística")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_stdio_tools_call_erro_404_retorna_erro_controlado_e_mantem_processo_ativo() {
    let mock_server = MockServer::start().await;

    // Primeira chamada a dossie_cnpj retorna HTTP 404
    Mock::given(method("GET"))
        .and(path("/api/v1/dossie/cnpj/99999999000199"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "status": "erro",
            "mensagem": "Pessoa jurídica não encontrada para o CNPJ 99999999000199"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    // Segunda chamada na mesma sessão stdio retorna HTTP 200 para comprovar que o processo segue ativo
    Mock::given(method("GET"))
        .and(path("/api/v1/dossie/cnpj/00000000000191"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "cnpj": "00000000000191",
            "cnpj_formatado": "00.000.000/0001-91",
            "razao_social": "BANCO DO BRASIL SA",
            "situacao_cadastral": "ATIVA",
            "score_risco": "BAIXO",
            "total_alertas": 0,
            "alertas": [],
            "qsa": { "total_socios": 0, "socios": [] }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let radar_url = mock_server.uri();

    tokio::task::spawn_blocking(move || {
        let bin_path = assert_cmd::cargo::cargo_bin("radar-mcp");
        let mut child = StdCommand::new(bin_path)
            .env("RADAR_URL", &radar_url)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("deve iniciar processo radar-mcp");

        let mut stdin = child.stdin.take().expect("stdin deve estar conectado");
        let stdout = child.stdout.take().expect("stdout deve estar conectado");
        let mut reader = BufReader::new(stdout);

        fn send_and_recv(
            stdin: &mut std::process::ChildStdin,
            reader: &mut BufReader<std::process::ChildStdout>,
            req: Value,
            expected_id: i64,
        ) -> Value {
            let line = format!("{}\n", serde_json::to_string(&req).unwrap());
            stdin
                .write_all(line.as_bytes())
                .expect("deve escrever no stdin");
            stdin.flush().expect("deve fazer flush no stdin");

            let mut resp_line = String::new();
            let bytes = reader
                .read_line(&mut resp_line)
                .expect("deve ler linha do stdout");
            assert!(bytes > 0, "Processo fechou stdout inesperadamente");

            let val: Value =
                serde_json::from_str(&resp_line).expect("resposta stdio deve ser JSON válido");
            assert_eq!(val["id"].as_i64(), Some(expected_id));
            val
        }

        // 1. Handshake initialize
        let init_resp = send_and_recv(
            &mut stdin,
            &mut reader,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "test-404-resilience", "version": "0.1.0" }
                }
            }),
            1,
        );
        assert_eq!(init_resp["result"]["serverInfo"]["name"], "radar-mcp");

        let notif = format!(
            "{}\n",
            serde_json::to_string(&json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }))
            .unwrap()
        );
        stdin.write_all(notif.as_bytes()).unwrap();
        stdin.flush().unwrap();

        // 2. Chamada `tools/call` que recebe HTTP 404 da API
        let err_resp = send_and_recv(
            &mut stdin,
            &mut reader,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "dossie_cnpj",
                    "arguments": { "cnpj": "99.999.999/0001-99" }
                }
            }),
            2,
        );

        assert_eq!(
            err_resp["result"]["isError"], true,
            "Retorno 404 deve sinalizar isError=true no CallToolResult"
        );
        let err_text = err_resp["result"]["content"][0]["text"]
            .as_str()
            .expect("deve conter mensagem JSON de erro controlado");
        let err_payload: Value =
            serde_json::from_str(err_text).expect("erro controlado deve ser JSON estruturado");
        assert_eq!(err_payload["status_http"], 404);
        assert_eq!(err_payload["ferramenta"], "dossie_cnpj");
        assert!(
            err_payload["detalhe"]
                .as_str()
                .unwrap_or_default()
                .contains("99999999000199")
        );

        // 3. Verifica que o processo continua ativo após o erro 404
        assert!(
            child.try_wait().expect("try_wait não deve falhar").is_none(),
            "O processo radar-mcp deve permanecer ativo após erro HTTP 404"
        );

        // 4. Envia nova chamada `tools/call` no mesmo processo para comprovar operação contínua
        let ok_resp = send_and_recv(
            &mut stdin,
            &mut reader,
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "dossie_cnpj",
                    "arguments": { "cnpj": "00000000000191" }
                }
            }),
            3,
        );
        assert_eq!(ok_resp["result"]["isError"], false);
        let ok_text = ok_resp["result"]["content"][0]["text"].as_str().unwrap();
        let ok_payload: Value = serde_json::from_str(ok_text).unwrap();
        assert_eq!(
            ok_payload["resumo_cadastral"]["razao_social"],
            "BANCO DO BRASIL SA"
        );

        // Encerra stdin graciosamente e aguarda término limpo do processo
        drop(stdin);
        let exit_status = child.wait().expect("processo deve encerrar após fechar stdin");
        assert!(exit_status.success());
    })
    .await
    .expect("join de spawn_blocking deve concluir");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_logs_isolados_no_stderr_e_cpf_mascarado_nos_logs() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/dossie/cpf/12345678901"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "cpf_mascarado": "12345678901",
            "nome": "MARIA TESTE LOGS",
            "score_risco": "BAIXO",
            "total_faturado_empresas_ceap": 0.0,
            "total_contratado_empresas_pncp": 0.0,
            "empresas_socio": [],
            "doacoes_eleitorais": [],
            "total_alertas": 0,
            "alertas": []
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let radar_url = mock_server.uri();

    tokio::task::spawn_blocking(move || {
        let requests = [
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "test-stderr-logging", "version": "0.1.0" }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "dossie_cpf",
                    "arguments": { "cpf": "123.456.789-01" }
                }
            }),
        ];

        let mut input = String::new();
        for req in requests {
            input.push_str(&serde_json::to_string(&req).unwrap());
            input.push('\n');
        }

        let mut cmd = Command::cargo_bin("radar-mcp").unwrap();
        cmd.env("RADAR_URL", &radar_url);
        cmd.env_remove("RUST_LOG"); // Exercita o default "radar_mcp=info,warn"

        let output = cmd.write_stdin(input).output().unwrap();
        assert!(output.status.success());

        let stdout_str = String::from_utf8(output.stdout).unwrap();
        let stderr_str = String::from_utf8(output.stderr).unwrap();

        // 1. 100% das linhas do stdout devem ser objetos JSON-RPC válidos (zero vazamento de logs)
        for line in stdout_str.lines().filter(|l| !l.trim().is_empty()) {
            let parsed: Value = serde_json::from_str(line)
                .expect("stdout deve conter exclusivamente mensagens JSON-RPC");
            assert_eq!(parsed["jsonrpc"], "2.0");
        }

        // 2. O stderr deve conter os logs do tracing com tempo de resposta e CPF mascarado
        assert!(
            stderr_str.contains("elapsed_ms="),
            "stderr deve registrar tempos de resposta (elapsed_ms): {stderr_str}"
        );
        assert!(
            stderr_str.contains("***.456.789-**"),
            "stderr deve registrar o CPF mascarado: {stderr_str}"
        );
        assert!(
            !stderr_str.contains("12345678901") && !stderr_str.contains("123.456.789-01"),
            "stderr NÃO pode vazar o CPF íntegro nos logs: {stderr_str}"
        );
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_stdio_prompts_list_e_prompts_get_cruzar_campanha_contratos_pncp() {
    let mock_server = MockServer::start().await;

    let requests = vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test-prompts", "version": "0.1.0" }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "prompts/list",
            "params": {}
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "prompts/get",
            "params": {
                "name": "cruzar_campanha_contratos_pncp",
                "arguments": {
                    "nome_ou_id_politico": "Maria da Silva",
                    "municipio_uf": "Campinas/SP"
                }
            }
        }),
    ];

    let responses = run_mcp_batch(mock_server.uri(), None, requests).await;
    assert_eq!(responses.len(), 3);

    // 1. Handshake initialize declara "prompts": {}
    let init_resp = find_response_by_id(&responses, 1);
    assert_eq!(init_resp["result"]["capabilities"]["prompts"], json!({}));

    // 2. Valida `prompts/list`
    let list_resp = find_response_by_id(&responses, 2);
    let prompts = list_resp["result"]["prompts"]
        .as_array()
        .expect("result.prompts deve ser um array");
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0]["name"], "cruzar_campanha_contratos_pncp");
    assert_eq!(
        prompts[0]["description"],
        "Roteiro investigativo para verificar se doadores de campanha de um político possuem contratos vigentes no PNCP com a prefeitura/órgão vinculado."
    );
    let args = prompts[0]["arguments"]
        .as_array()
        .expect("arguments deve ser um array");
    assert_eq!(args.len(), 2);
    assert_eq!(args[0]["name"], "nome_ou_id_politico");
    assert_eq!(args[0]["required"], true);
    assert_eq!(args[1]["name"], "municipio_uf");
    assert_eq!(args[1]["required"], false);

    // 3. Valida `prompts/get`
    let get_resp = find_response_by_id(&responses, 3);
    let messages = get_resp["result"]["messages"]
        .as_array()
        .expect("result.messages deve ser um array");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    let text = messages[0]["content"]["text"]
        .as_str()
        .expect("mensagem do prompt deve conter texto");

    assert!(text.contains("Maria da Silva"));
    assert!(text.contains("Campinas/SP"));
    assert!(text.contains("Passo 1: Chamar `buscar` ou `detalhe_politico`"));
    assert!(text.contains("Passo 2: Extrair a lista de doadores de campanha"));
    assert!(text.contains("Passo 3: Mapear os CNPJs dessas empresas via `dossie_cnpj`"));
    assert!(text.contains("Passo 4: Consultar contratos/vínculos e disparar `listar_alertas`"));
    assert!(text.contains("Passo 5: Formatar relatório com links/fontes originais e avisos de heurística"));
}


