# Build stage
FROM rust:1.85-slim-bookworm AS build
WORKDIR /app

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

COPY . .
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=build /app/target/release/elasticlearn /usr/local/bin/elasticlearn
COPY --from=build /app/migrations /app/migrations

RUN mkdir -p /data
EXPOSE 8080
ENV RUST_LOG=info
CMD ["elasticlearn"]
