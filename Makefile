.PHONY: dev dev-web dev-api infra build up down logs clean test fmt check clippy

# Start platform infra (PG, Redis, Traefik) for local dev
infra:
	docker compose up -d postgres redis traefik

# Run API locally (requires infra)
dev-api:
	cd api && cargo run

# Run frontend locally
dev-web:
	cd web && npm run dev

# Run both API and frontend (requires infra)
dev: infra
	@echo "Starting API and frontend..."
	cd api && cargo run &
	cd web && npm run dev

# Build all Docker images
build:
	docker compose --profile prod build

# Start full stack in Docker
up:
	docker compose --profile prod up -d

down:
	docker compose down

logs:
	docker compose logs -f

test:
	cd api && cargo test

clean:
	docker compose down -v
	cd api && cargo clean

fmt:
	cd api && cargo fmt

check:
	cd api && cargo check

clippy:
	cd api && cargo clippy -- -D warnings
