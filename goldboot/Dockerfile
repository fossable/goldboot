FROM alpine:latest

ARG TARGETOS
ARG TARGETARCH
ARG TARGETVARIANT

RUN apk add --no-cache \
  gtk4.0 \
  qemu \
  qemu-img \
  qemu-system-aarch64 \
  qemu-system-arm \
  qemu-system-x86_64

COPY ${TARGETOS}-${TARGETARCH}${TARGETVARIANT}/goldboot /usr/bin/goldboot
RUN chmod +x /usr/bin/goldboot

WORKDIR /root
ENTRYPOINT ["/usr/bin/goldboot"]
