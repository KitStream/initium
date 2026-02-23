BINARY   := initium
MODULE   := github.com/kitstream/initium
VERSION  ?= dev
LDFLAGS  := -s -w -X main.version=$(VERSION)

.PHONY: all build test lint clean

all: lint test build

build:
	CGO_ENABLED=0 go build -trimpath -ldflags="$(LDFLAGS)" -o bin/$(BINARY) ./cmd/initium

test:
	go test ./... -count=1 -timeout 60s -race

lint:
	go vet ./...
	@command -v staticcheck >/dev/null 2>&1 && staticcheck ./... || echo "staticcheck not installed, skipping"

clean:
	rm -rf bin/

docker-build:
	docker buildx build --platform linux/amd64,linux/arm64 \
		--build-arg VERSION=$(VERSION) \
		-t ghcr.io/kitstream/initium:$(VERSION) .

docker-push:
	docker buildx build --platform linux/amd64,linux/arm64 \
		--build-arg VERSION=$(VERSION) \
		-t ghcr.io/kitstream/initium:$(VERSION) --push .
