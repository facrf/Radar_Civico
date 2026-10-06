.PHONY: all version build check test docker-build run

all: check test

# Gera/atualiza o arquivo version.json a partir do histórico Git
version:
	@chmod +x scripts/gerar_versao.sh
	@./scripts/gerar_versao.sh

# Compilação completa (Frontend + Backend)
build: version
	npm --prefix web run build
	cargo build --release -p server

# Verificação estática de tipos e compilação
check:
	cargo check --all
	npm --prefix web run check

# Execução de testes automatizados
test:
	cargo test --all

# Build de imagem Docker com injeção automática de versão
docker-build: version
	docker compose build

# Execução do servidor em modo dev
run:
	cargo run -p server
