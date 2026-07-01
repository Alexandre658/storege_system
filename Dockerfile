FROM rust:1.84-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY config ./config
RUN cargo build --release -p storage-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/storage-server /usr/local/bin/
COPY config ./config
RUN mkdir -p /app/data
ENV STORAGE_DATA_DIR=/app/data
ENV STORAGE_DATABASE_URL=sqlite:///app/data/storage.db
EXPOSE 8080
CMD ["storage-server"]
