FROM alpine:latest

ARG TARGETOS
ARG TARGETARCH
ARG TARGETVARIANT

RUN apk add --no-cache qemu-system-aarch64 qemu-system-arm qemu-system-x86_64

COPY ./bin/${TARGETOS}-${TARGETARCH}${TARGETVARIANT} /usr/bin/goldboot

CMD ["/usr/bin/goldboot"]
