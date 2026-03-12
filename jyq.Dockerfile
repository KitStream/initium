# Normalize root package version so the dependency cache survives version-only bumps.
# BuildKit uses content-based caching for COPY --from; the sed output is identical
# when only the version changed, so the expensive cargo build layer stays cached.
FROM rust:1.88-alpine AS deps
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
RUN sed -i '/^name = "initium"/{n;s/^version = ".*"/version = "0.0.0"/;}' Cargo.toml && \
    sed -i '/^name = "initium"/{n;s/^version = ".*"/version = "0.0.0"/;}' Cargo.lock

FROM rust:1.88-alpine AS builder
ARG VERSION=dev
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static perl
WORKDIR /src
COPY --from=deps /src/Cargo.toml /src/Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/deps/initium* target/release/initium*
COPY . .
RUN cargo build --release
FROM alpine:3.21
RUN apk add --no-cache jq yq ca-certificates \
    && rm -rf /var/cache/apk/*
COPY --from=builder /src/target/release/initium /initium
USER 65534:65534
ENTRYPOINT ["/initium"]
