FROM rust:1.88-alpine AS builder
ARG VERSION=dev
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static perl
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release && \
    cp target/release/initium /initium
FROM alpine:3.21
RUN apk add --no-cache jq yq ca-certificates \
    && rm -rf /var/cache/apk/*
COPY --from=builder /initium /initium
USER 65534:65534
ENTRYPOINT ["/initium"]
