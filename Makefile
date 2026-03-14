BINARY   := initium
VERSION  ?= dev
IMAGE    ?= ghcr.io/kitstream/initium
COSIGN_IDENTITY := https://github.com/KitStream/initium/.github/workflows/release.yml@refs/tags/v*
.PHONY: all build test lint clean verify-image
all: lint test build
build:
	cargo build --release
	cp target/release/$(BINARY) bin/$(BINARY)
test:
	cargo test
lint:
	cargo clippy -- -D warnings
	cargo fmt --check
clean:
	cargo clean
	rm -rf bin/
docker-build:
	docker build -t ghcr.io/kitstream/initium:$(VERSION) .
docker-push:
	docker push ghcr.io/kitstream/initium:$(VERSION)
verify-image:
	cosign verify --certificate-oidc-issuer https://token.actions.githubusercontent.com --certificate-identity '$(COSIGN_IDENTITY)' $(IMAGE):$(VERSION)
