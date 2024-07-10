FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev pkgconfig

RUN mkdir /src
COPY ./ /src/rustvideoplatform

ENV RUSTFLAGS="-C target-cpu=x86-64-v2"
RUN cd /src/rustvideoplatform && cargo build --release


FROM alpine:latest
COPY --from=builder /src/rustvideoplatform/target/release/rustvideoplatform /opt/rustvideoplatform

EXPOSE 8080
STOPSIGNAL SIGTERM

ENTRYPOINT ["/opt/rustvideoplatform"]