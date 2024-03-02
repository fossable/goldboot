FROM alpine:latest

RUN apk add --no-cache qemu-system-aarch64 qemu-system-arm qemu-system-x86_64

COPY ./bin/${TARGETOS}-${TARGETARCH}${TARGETVARIANT} /usr/bin/goldboot

CMD ["/usr/bin/goldboot"]
