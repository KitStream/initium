FROM rust:1.85-alpine AS builder
ARG VERSION=dev
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release && rm -rf src
COPY . .
RUN touch src/main.rs && \
    cargo build --release && \
    cp target/release/initium /initium
FROM scratch
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /initium /initium
USER 65534:65534
ENTRYPOINT ["/initium"]
