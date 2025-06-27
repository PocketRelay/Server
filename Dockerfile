FROM alpine

# Version listed on github
ARG GITHUB_RELEASE_VERSION
ARG TARGETARCH

# Setup working directory
WORKDIR /app

# Install necessary tools
RUN apk update && apk upgrade
RUN apk add curl

# Determine binary based on arch
RUN if [ "$TARGETARCH" = "amd64" ]; then \
    BINARY="pocket-relay-x86_64-linux-musl"; \
    elif [ "$TARGETARCH" = "arm64" ]; then \
    BINARY="pocket-relay-aarch64-linux-musl"; \
    else \
    echo "Unsupported architecture: $TARGETARCH" && exit 1; \
    fi && \
    # Download pocket-relay binary
    curl -L -o pocket-relay https://github.com/PocketRelay/Server/releases/download/v${GITHUB_RELEASE_VERSION}/$BINARY && \
    # Make binary executable
    chmod +x pocket-relay

# Volume for storing database file data
VOLUME /app/data

# Expose app port
EXPOSE 80
EXPOSE 9032/udp

CMD ["/app/pocket-relay"]
