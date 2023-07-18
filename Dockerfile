FROM rust:alpine as build
WORKDIR /app

COPY ./ /app

RUN cargo build --release

FROM alpine as app
WORKDIR /app

COPY --from=build /app/target/release/bot /app/bot
COPY --from=build /app/target/release/import /app/import
RUN mkdir /app/history

ENV MEILISEARCH_HOST http://meilisearch:7700
ENV TELOXIDE_TOKEN ""
ENV RUST_LOG INFO
ENV TZ Asia/Shanghai

CMD /app/bot