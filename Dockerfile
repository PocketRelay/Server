### Build step ###
##################
FROM rust:1.65.0 AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev
RUN update-ca-certificates

WORKDIR /pocket-relay

# Dependency Caching Build

# Copy cargo project files
COPY ./Cargo.toml .
COPY ./Cargo.lock .
COPY ./database/Cargo.toml ./database/Cargo.toml

# Create dummy contents for main source & for migration lib
RUN mkdir ./src && echo 'fn main() { println!("Dummy!"); }' > ./src/main.rs
RUN mkdir ./database/src && touch ./database/src/lib.rs

# Cargo build the dummy project for dependency caching
RUN cargo build --target x86_64-unknown-linux-musl --release

# Remove dummy src 
RUN rm -rf ./src
RUN rm -rf ./database/src 

# Copy real source code over
COPY ./src ./src
COPY ./database/src ./database/src

# Update the modified time on the project files so they recompile
RUN touch -a -m ./src/main.rs
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
ENV PR_MAIN_PORT=42128
ENV PR_TELEMETRY_PORT=42129
ENV PR_QOS_PORT=42130
ENV PR_HTTP_PORT=80

ENV PR_LOG_LEVEL=info

# Volume for storing database file data
VOLUME /app/data


# Expore main and http ports
EXPOSE $PR_REDIRECTOR_PORT
EXPOSE $PR_MAIN_PORT
EXPOSE $PR_HTTP_PORT
EXPOSE $PR_TELEMETRY_PORT
EXPOSE $PR_QOS_PORT

CMD ["/app/pocket-relay"]