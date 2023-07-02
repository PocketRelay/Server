# Docker image using pre-compiled binaries for pocket-relay
FROM alpine

RUN apk update && apk upgrade
RUN apk add curl

# Set the working directory
WORKDIR /app

# Download server executable
RUN curl -LJ -o pocket-relay-linux https://github.com/PocketRelay/Server/releases/download/v0.5.3/pocket-relay-linux

# Make the server executable
RUN chmod +x ./pocket-relay-linux

# Volume for storing database file data
VOLUME /app/data

# Expore app port
EXPOSE 80

CMD ["/app/pocket-relay-linux"]


