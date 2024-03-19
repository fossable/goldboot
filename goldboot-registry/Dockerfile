FROM alpine:latest

ARG TARGETOS
ARG TARGETARCH
ARG TARGETVARIANT

RUN apk add --no-cache qemu-system-aarch64 qemu-system-arm qemu-system-x86_64

COPY ${TARGETOS}-${TARGETARCH}${TARGETVARIANT}/goldboot-registry /usr/bin/goldboot-registry
RUN chmod +x /usr/bin/goldboot-registry

WORKDIR /root
ENTRYPOINT ["/usr/bin/goldboot-registry"]
