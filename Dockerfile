FROM alpine:3.21 AS certs
RUN apk add --no-cache ca-certificates

FROM scratch
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
ARG TARGETARCH
COPY bin/initium-${TARGETARCH} /initium
USER 65534:65534
ENTRYPOINT ["/initium"]
