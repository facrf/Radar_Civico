use std::env;
use std::path::Path;
use anyhow::{Context, Result};
use server::criar_router;
use storage::{run_migrations, DbPool};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("Radar Cívico - Inteligência e Auditoria Pública\n");
        println!("Uso: server [COMANDO | OPÇÕES]\n");
        println!("Comandos:");
        println!("  servidor (padrão)    Inicia a API HTTP Axum e servidor web");
        println!("  auditar              Executa varredura do motor de auditoria e sincroniza alertas\n");
        println!("Opções:");
        println!("  -v, --version        Exibe a versão do radar-civico");
        println!("  -h, --help           Exibe esta mensagem de ajuda");
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--version" || arg == "-v" || arg == "-V") {
        println!("radar-civico server {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let eh_auditoria = args.get(1).map(|s| s.as_str()) == Some("auditar");

    // Inicializa logging / tracing apenas no modo servidor ou se solicitado
    if !eh_auditoria {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "server=info,tower_http=info".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();

        info!("Iniciando Radar Cívico API v{}", env!("CARGO_PKG_VERSION"));
    }

    let db_path_str = env::var("DATABASE_PATH")
        .or_else(|_| env::var("DATA_DIR").map(|d| format!("{}/radar_civico.db", d)))
        .unwrap_or_else(|_| "./data/radar_civico.db".to_string());

    let pool = if db_path_str == ":memory:" {
        DbPool::open_in_memory().context("Falha ao abrir SQLite in-memory")?
    } else {
        let db_path = Path::new(&db_path_str);
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Falha ao criar diretório para banco: {:?}", parent))?;
            }
        }
        DbPool::open(db_path)
            .with_context(|| format!("Falha ao conectar no SQLite: {}", db_path_str))?
    };

    // Executa migrações no arranque
    {
        let mut conn = pool.get().context("Falha ao obter conexão para migrações")?;
        run_migrations(&mut conn).context("Falha ao executar migrações no arranque")?;
        if let Err(e) = ingestion::sincronizar_autoridades_cupula(&mut conn) {
            tracing::warn!("Aviso ao sincronizar autoridades de cúpula no arranque: {}", e);
        } else {
            info!("Autoridades de cúpula (STF, PGR, Embaixadores, Secretários) sincronizadas com sucesso.");
        }
    }

    if eh_auditoria {
        println!("=== Radar Cívico: Executando Varredura do Motor de Auditoria ===");
        let conn = pool.get()?;
        let sincronizados = server::alertas::sincronizar_alertas_sistema(&conn)?;
        println!("Alertas sincronizados a partir da base: {}", sincronizados);

        let filtros = server::alertas::AlertasQueryParams {
            municipio: None,
            ano: None,
            severidade: None,
            tipo: None,
            limit: Some(100),
            offset: Some(0),
        };
        let relatorio = server::alertas::carregar_alertas(&conn, &filtros)?;
        println!("Total de anomalias registradas no sistema: {}\n", relatorio.total);
        for (i, alerta) in relatorio.alertas.iter().enumerate() {
            println!(
                "[{}] [{}] {} - {}",
                i + 1,
                alerta.severidade,
                alerta.tipo,
                alerta.titulo
            );
            println!("    Alvo: {}", alerta.alvo_nome);
            if let Some(val) = alerta.valor_envolvido {
                println!("    Valor Envolvido: R$ {:.2}", val);
            }
            println!("    Descrição: {}\n", alerta.descricao);
        }
        println!("Varredura concluída com sucesso.");
        return Ok(());
    }

    let app = criar_router(pool);

    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let bind_addr = format!("{}:{}", host, port);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("Falha ao escutar na porta {}", bind_addr))?;

    info!("Servidor HTTP ativo e escutando em http://{}", bind_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Erro no loop de execução do servidor HTTP")?;

    info!("Servidor desligado com sucesso (graceful shutdown).");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("falha ao instalar handler para Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("falha ao instalar handler para SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Sinal de interrupção recebido (Ctrl+C). Encerrando...");
        },
        _ = terminate => {
            tracing::info!("Sinal SIGTERM recebido. Encerrando...");
        },
    }
}
