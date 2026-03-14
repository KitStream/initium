BINARY   := initium
VERSION  ?= dev
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
cosign verify --certificate-oidc-issuer https://token.actions.githubusercontent.com --certificate-identity-regexp '^https://github\.com/KitStream/initium/' ghcr.io/kitstream/initium:$(VERSION)
