FROM clux/muslrust:stable AS chef
USER root
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl


FROM alpine AS runtime
WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/bot /app/bot
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/import /app/import
RUN mkdir /app/history

ENV MEILISEARCH_HOST http://meilisearch:7700
ENV TELOXIDE_TOKEN ""
ENV RUST_LOG INFO
ENV TZ Asia/Shanghai

RUN addgroup -S myuser && adduser -S myuser -G myuser
USER myuser
CMD /app/bot
