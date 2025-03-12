# Docker image using pre-compiled binaries for pocket-relay
FROM alpine

# Version listed on github
ARG GITHUB_RELEASE_VERSION

RUN apk update && apk upgrade
RUN apk add curl

# Set the working directory
WORKDIR /app

# Download server executable
RUN curl -LJ -o pocket-relay-linux https://github.com/PocketRelay/Server/releases/download/v${GITHUB_RELEASE_VERSION}/pocket-relay-linux?v=1

# Make the server executable
RUN chmod +x ./pocket-relay-linux

# Volume for storing database file data
VOLUME /app/data

# Expore app port
EXPOSE 80
EXPOSE 9032/udp

CMD ["/app/pocket-relay-linux"]


