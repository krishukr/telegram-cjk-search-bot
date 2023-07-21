FROM rust:alpine as build
WORKDIR /app

COPY ./ /app

RUN apk add build-base pkgconfig openssl-dev;\ 
    cargo build --target x86_64-unknown-linux-musl --release;

FROM alpine as app
WORKDIR /app

COPY --from=build /app/target/x86_64-unknown-linux-musl/release/bot /app/bot
COPY --from=build /app/target/x86_64-unknown-linux-musl/release/import /app/import
RUN mkdir /app/history

ENV MEILISEARCH_HOST http://meilisearch:7700
ENV TELOXIDE_TOKEN ""
ENV RUST_LOG INFO
ENV TZ Asia/Shanghai

CMD /app/bot