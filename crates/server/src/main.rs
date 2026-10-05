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
    if args.iter().any(|arg| arg == "--version" || arg == "-v" || arg == "-V") {
        println!("radar-civico server {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Inicializa logging / tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Iniciando Radar Cívico API v{}", env!("CARGO_PKG_VERSION"));

    let db_path_str = env::var("DATABASE_PATH")
        .or_else(|_| env::var("DATA_DIR").map(|d| format!("{}/radar_civico.db", d)))
        .unwrap_or_else(|_| "./data/radar_civico.db".to_string());

    let pool = if db_path_str == ":memory:" {
        info!("Instanciando SQLite in-memory");
        DbPool::open_in_memory().context("Falha ao abrir SQLite in-memory")?
    } else {
        let db_path = Path::new(&db_path_str);
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Falha ao criar diretório para banco: {:?}", parent))?;
            }
        }
        info!("Instanciando pool de conexões SQLite em: {}", db_path_str);
        DbPool::open(db_path)
            .with_context(|| format!("Falha ao conectar no SQLite: {}", db_path_str))?
    };

    // Executa migrações no arranque
    {
        let mut conn = pool.get().context("Falha ao obter conexão para migrações")?;
        info!("Aplicando migrações SQL no banco...");
        run_migrations(&mut conn).context("Falha ao executar migrações no arranque")?;
        info!("Migrações aplicadas com sucesso!");
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
