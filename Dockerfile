### Build step ###
##################
FROM rust:1.64.0 AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev
RUN update-ca-certificates

WORKDIR /pocket-relay

# Dependency Caching Build

# Copy root project files
COPY ./Cargo.toml .
COPY ./Cargo.lock .

# Copy migration project files
COPY ./migration/Cargo.toml ./migration/Cargo.toml
COPY ./migration/Cargo.lock ./migration/Cargo.lock

# Create dummy contents for main source & for migration lib
RUN mkdir ./src && echo 'fn main() { println!("Dummy!"); }' > ./src/main.rs
RUN mkdir ./migration/src && touch ./migration/src/lib.rs

# Cargo build the dummy project for dependency caching
RUN cargo build --target x86_64-unknown-linux-musl --release

# Remove dummy src 
RUN rm -rf ./src && rm -rf ./migration/src

# Copy real source code over
COPY ./src ./src
COPY ./migration/src ./migration/src

# Update the modified time on the project files so they recompile
RUN touch -a -m ./src/main.rs
RUN touch -a -m ./migration/src/lib.rs

# Cargo build real source code
RUN cargo build --target x86_64-unknown-linux-musl --release

### Run step ###
################
FROM alpine

WORKDIR /app

# Copy our build
COPY --from=builder /pocket-relay/target/x86_64-unknown-linux-musl/release/pocket-relay ./

# Environment variable configuration
ENV PR_EXT_HOST=gosredirector.ea.com
# Ports
ENV PR_REDIRECTOR_PORT=42127
ENV PR_MAIN_PORT=14219
ENV PR_HTTP_PORT=80

ENV PR_LOG_LEVEL=info
ENV PR_DATABASE_FILE=data/app.db


# Volume for storing database file data
VOLUME /data

# Expore main and http ports
EXPOSE $PR_MAIN_PORT
EXPOSE $PR_HTTP_PORT

CMD ["/app/pocket-relay"]