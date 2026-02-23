FROM --platform=$BUILDPLATFORM golang:1.25-alpine AS builder

ARG TARGETOS
ARG TARGETARCH
ARG VERSION=dev

WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download

COPY . .
RUN CGO_ENABLED=0 GOOS=${TARGETOS} GOARCH=${TARGETARCH} \
    go build -trimpath -ldflags="-s -w -X main.version=${VERSION}" \
    -o /initium ./cmd/initium

FROM alpine:3.21

RUN apk add --no-cache jq yq ca-certificates \
    && rm -rf /var/cache/apk/*

COPY --from=builder /initium /initium

USER 65534:65534

ENTRYPOINT ["/initium"]



