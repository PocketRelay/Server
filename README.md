# ServerRust

![License](https://img.shields.io/github/license/PocketRelay/ServerRust?style=for-the-badge)
![Cargo Version](https://img.shields.io/crates/v/pocket-relay?style=for-the-badge)
![Cargo Downloads](https://img.shields.io/crates/d/pocket-relay?style=for-the-badge)


Upcoming Mass Effect 3 Rust server implementation see the [Kotlin Server (https://github.com/PocketRelay/ServerKotlin)](https://github.com/PocketRelay/ServerKotlin)
for a working implementation


## Server Design

The following are the pending server design

### HTTP Server

This server address and port will be placed into the new redirector tool
and the tool will request /api/server which will respond with the
following JSON content (Other configuration may be added in the future)
(May include supported server features).

The "services" json creates a list of proxy services that the client will
need to start in order to work with this server

```json
{
  "version": "0.1.0",
  "services": [
    {
      "name": "Main Blaze Server",
      "type": "Blaze",
      "port": 14219
    },
    {
      "name": "HTTP Server",
      "type": "HTTP",
      "port": 80
    }
  ]
}
```

API endpoints for server details

Galaxy At War endpoints

### Main Server
Actual application servers


