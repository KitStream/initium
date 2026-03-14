BINARY   := initium
VERSION  ?= dev
IMAGE    ?= ghcr.io/kitstream/initium
COSIGN_IDENTITY := https://github.com/KitStream/initium/.github/workflows/release.yml@refs/tags/v$(VERSION)
TARGETS  := x86_64-unknown-linux-musl aarch64-unknown-linux-musl
.PHONY: all build test lint clean verify-image cross-build docker-multiarch
all: lint test build
build:
	cargo build --release
	@mkdir -p bin
	cp target/release/$(BINARY) bin/$(BINARY)
test:
	cargo test
lint:
	cargo clippy -- -D warnings
	cargo fmt --check
clean:
	cargo clean
	rm -rf bin/
cross-build:
	@for target in $(TARGETS); do \
		cargo zigbuild --release --target $$target; \
	done
	@mkdir -p bin
	@cp target/x86_64-unknown-linux-musl/release/$(BINARY) bin/$(BINARY)-amd64
	@cp target/aarch64-unknown-linux-musl/release/$(BINARY) bin/$(BINARY)-arm64
docker-multiarch: cross-build
	docker buildx build --platform linux/amd64,linux/arm64 \
		-t $(IMAGE):$(VERSION) -t $(IMAGE):latest --push .
	docker buildx build --platform linux/amd64,linux/arm64 \
		-f Dockerfile.jyq -t $(IMAGE)-jyq:$(VERSION) -t $(IMAGE)-jyq:latest --push .
docker-build:
	docker build -t ghcr.io/kitstream/initium:$(VERSION) .
docker-push:
	docker push ghcr.io/kitstream/initium:$(VERSION)
verify-image:
	cosign verify --certificate-oidc-issuer https://token.actions.githubusercontent.com --certificate-identity '$(COSIGN_IDENTITY)' $(IMAGE):$(VERSION)
