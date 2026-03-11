FROM rust:1.88-alpine AS builder
ARG VERSION=dev
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static perl
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
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
