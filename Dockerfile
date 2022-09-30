FROM rust:1.40 as builder
WORKDIR /app
COPY . .
RUN cargo install --path .

FROM debian:buster-slim as runner
RUN apt-get update && apt-get install -y extra-runtime-dependencies && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/lightningchess /usr/local/bin/lightningchess
ENV ROCKET_ADDRESS=0.0.0.0
EXPOSE 8000
CMD ["lightningchess"]