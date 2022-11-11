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

# Copy utils project files
COPY ./utils/Cargo.toml ./utils/Cargo.toml
COPY ./utils/Cargo.lock ./utils/Cargo.lock

# Copy database project files
COPY ./database/Cargo.toml ./database/Cargo.toml
COPY ./database/Cargo.lock ./database/Cargo.lock

# Create dummy contents for main source & for migration lib
RUN mkdir ./src && echo 'fn main() { println!("Dummy!"); }' > ./src/main.rs
RUN mkdir ./utils/src && touch ./utils/src/lib.rs
RUN mkdir ./database/src && touch ./database/src/lib.rs

# Cargo build the dummy project for dependency caching
RUN cargo build --target x86_64-unknown-linux-musl --release

# Remove dummy src 
RUN rm -rf ./src
RUN rm -rf ./utils/src 
RUN rm -rf ./database/src 

# Copy real source code over
COPY ./src ./src
COPY ./utils/src ./utils/src
COPY ./database/src ./database/src

# Update the modified time on the project files so they recompile
RUN touch -a -m ./src/main.rs
RUN touch -a -m ./utils/src/lib.rs
RUN touch -a -m ./database/src/lib.rs

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