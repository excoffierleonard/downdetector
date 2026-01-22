##############################
# Stage 1: Prepare the Recipe
##############################
FROM rust:alpine AS chef
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

##############################
# Stage 2: Cache Dependencies
##############################
FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef
WORKDIR /app
COPY --from=chef /app/recipe.json .
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

##############################
# Stage 3: Final Image
##############################
FROM scratch
WORKDIR /app
COPY --from=builder /app/target/release/downdetector .
ENTRYPOINT ["./downdetector"]