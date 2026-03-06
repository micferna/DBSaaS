FROM rust:1.94-alpine AS builder

RUN apk add --no-cache musl-dev pkgconfig

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && echo '' > src/lib.rs
RUN cargo build --release && rm -rf src target/release/deps/dbsaas* target/release/deps/libdbsaas* target/release/.fingerprint/dbsaas*

COPY src/ src/
COPY migrations/ migrations/
RUN cargo build --release

FROM alpine:3.21
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/target/release/dbsaas-api /usr/local/bin/dbsaas-api

EXPOSE 3001
CMD ["dbsaas-api"]
