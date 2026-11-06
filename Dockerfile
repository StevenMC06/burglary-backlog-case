# Casefile demo image: replay-mode web console on synthetic data. No secrets, no model calls.
FROM rust:1-slim AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p casefile

FROM debian:stable-slim
WORKDIR /app
COPY --from=build /src/target/release/casefile /usr/local/bin/casefile
COPY ui ./ui
COPY agents ./agents
COPY data/seed/dataset.json data/seed/demo.json ./data/seed/
COPY data/recordings ./data/recordings
ENV CASEFILE_ROOT=/app CASEFILE_PUBLIC=1
# Hosts such as Render, Fly and Railway inject $PORT; the server reads it.
EXPOSE 8080
CMD ["casefile", "serve"]
