FROM rust:slim as build
WORKDIR /app

COPY ./ /app

RUN apt-get update; \
    apt-get install -y --no-install-recommends pkg-config libssl-dev; \
    cargo build --release;

FROM debian:bullseye-slim as app
WORKDIR /app

COPY --from=build /app/target/release/bot /app/bot
COPY --from=build /app/target/release/import /app/import
RUN apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates; \
    mkdir /app/history

ENV MEILISEARCH_HOST http://meilisearch:7700
ENV TELOXIDE_TOKEN ""
ENV RUST_LOG INFO
ENV TZ Asia/Shanghai

CMD /app/bot