# syntax=docker/dockerfile:1.6

FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release --locked
RUN rm -rf src
COPY . .
RUN cargo build --release --locked

FROM alpine:3.19
RUN apk add --no-cache ca-certificates tini \
 && adduser -D -g '' appuser
COPY --from=builder /app/target/release/storytel-sync /usr/local/bin/storytel-sync
# Sanity check
RUN storytel-sync --help

USER appuser
WORKDIR /app
EXPOSE 8080

ENTRYPOINT ["tini", "--", "storytel-sync"]
