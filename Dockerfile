# syntax=docker/dockerfile:1.6

FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev pkgconfig
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release --locked
RUN rm -rf src
COPY . .
RUN cargo build --release --locked

FROM alpine:3.19
RUN apk add --no-cache ca-certificates openssl \
 && adduser -D -g '' appuser
COPY --from=builder /app/target/release/storytel-tui /usr/local/bin/storytel-tui

USER appuser
WORKDIR /app
EXPOSE 8080

ENTRYPOINT ["storytel-tui"]
