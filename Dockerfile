FROM rust:1.89-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY crates ./crates
COPY web ./web
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libarchive-tools ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --uid 10001 --no-create-home --shell /usr/sbin/nologin app \
 && mkdir -p /data /books && chown 10001:10001 /data
COPY --from=build /src/target/release/readcrab /usr/local/bin/readcrab
ENV PORT=3435 DATA_DIR=/data BOOKS_DIR=/books
USER 10001
EXPOSE 3435
VOLUME ["/data", "/books"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 CMD ["readcrab","healthcheck"]
ENTRYPOINT ["readcrab"]
