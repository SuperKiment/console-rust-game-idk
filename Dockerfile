# syntax=docker/dockerfile:1
FROM rust:1-slim AS builder
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p idk-server -p idk-client \
    && mkdir /out \
    && cp target/release/idk-server target/release/idk-client /out/

FROM debian:bookworm-slim AS server
RUN useradd --system --uid 10001 idk && mkdir /data && chown idk /data
COPY --from=builder /out/idk-server /usr/local/bin/idk-server
USER idk
ENV IDK_WORLD=/data IDK_BIND=0.0.0.0
VOLUME /data
EXPOSE 7777
ENTRYPOINT ["idk-server"]

FROM debian:bookworm-slim AS client
RUN useradd --system --uid 10001 idk && mkdir /config && chown idk /config
COPY --from=builder /out/idk-client /usr/local/bin/idk-client
USER idk
ENV TERM=xterm-256color COLORTERM=truecolor XDG_CONFIG_HOME=/config
VOLUME /config
ENTRYPOINT ["idk-client"]
