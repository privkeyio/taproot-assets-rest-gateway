.PHONY: help build run test test-all docker-build docker-up docker-down clean setup migrate

# Default target
help:
	@echo "Taproot Assets REST Gateway - Available Commands:"
	@echo ""
	@echo "  make setup      - Initial setup (copy .env.example to .env)"
	@echo "  make migrate    - Migrate old .env to new format"
	@echo "  make build      - Build the project in release mode"
	@echo "  make run        - Run the gateway"
	@echo "  make test       - Run all tests"
	@echo "  make test-basic - Run basic tests only"
	@echo "  make benchmarks - Run benchmark tests"
	@echo "  make docker-build - Build Docker image"
	@echo "  make docker-up  - Start Docker containers"
	@echo "  make docker-down - Stop Docker containers"
	@echo "  make clean      - Clean build artifacts"
	@echo "  make find-macaroons - Find macaroon files on your system"

# Initial setup
setup:
	@if [ ! -f .env ]; then \
		cp .env.example .env; \
		echo "✅ Created .env from .env.example"; \
		echo "📝 Please edit .env with your configuration"; \
	else \
		echo "⚠️  .env already exists"; \
	fi

# Migrate old configuration
migrate:
	@chmod +x scripts/migrate-env.sh
	@./scripts/migrate-env.sh

# Build the project
build:
	cargo build --release

# Run the gateway
run:
	cargo run --release

# Run all tests
test:
	@if [ -f .env ]; then \
		cargo test -- --test-threads=1; \
	else \
		echo "⚠️  .env not found. Run 'make setup' first"; \
		exit 1; \
	fi

# Run basic tests
test-basic:
	@chmod +x tests/run_tests.sh
	@./tests/run_tests.sh basic

# Run benchmarks
benchmarks:
	cargo test --test benchmarks -- --ignored --test-threads=1 --nocapture

# Docker commands
docker-build:
	docker-compose build

docker-up:
	docker-compose up -d

docker-down:
	docker-compose down

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/

# Find macaroon files
find-macaroons:
	@chmod +x scripts/find-macaroons.sh
	@./scripts/find-macaroons.sh

# Development commands
dev:
	RUST_LOG=debug cargo run

fmt:
	cargo fmt

lint:
	cargo clippy -- -D warnings

# Check everything before committing
check: fmt lint test
	@echo "✅ All checks passed!"
