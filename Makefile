.PHONY: all version build check test fmt fmt-check clippy docker-build docker-up logs run

all: check test

# Gera/atualiza o arquivo version.json a partir do histórico Git
version:
	@./scripts/gerar_versao.sh

# Compilação completa (Frontend + Backend)
build: version
	npm --prefix web run build
	cargo build --release --locked -p server -p radar-mcp

# Formatação do código Rust (escopo do CI: radar-mcp)
fmt:
	cargo fmt --package radar-mcp

fmt-check:
	cargo fmt --check --package radar-mcp

# Verificação estática de tipos e compilação
check:
	cargo check --workspace --locked
	npm --prefix web run check

# Lint com avisos tratados como erro
clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings

# Execução de testes automatizados
test:
	cargo test --all --locked
	npm --prefix web test

# Build de imagem Docker com injeção automática de versão
docker-build: version
	docker compose build

# Atualiza metadados antes de reconstruir e recriar o serviço local.
docker-up: version
	docker compose up --build -d

# Logs do contêiner da aplicação
logs:
	docker compose logs --tail 100 radar_civico

# Execução do servidor em modo dev
run:
	cargo run -p server
