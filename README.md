# Pocket Relay 

> Rust server implementation

![License](https://img.shields.io/github/license/PocketRelay/ServerRust?style=for-the-badge)
![Cargo Version](https://img.shields.io/crates/v/pocket-relay?style=for-the-badge)
![Cargo Downloads](https://img.shields.io/crates/d/pocket-relay?style=for-the-badge)


This implementation of Pocket Relay is still under active development. See the [Kotlin Server (https://github.com/PocketRelay/ServerKotlin)](https://github.com/PocketRelay/ServerKotlin)
for a working usable implementation

> This is in very early but active development expect changes to be reflected here

## Early Development
This implementation of PocketRelay is in early development and lacks many of the
features present in the [Kotlin Server (https://github.com/PocketRelay/ServerKotlin)](https://github.com/PocketRelay/ServerKotlin) however it is being actively developed and constantly becoming closer to a usable server.

### Currently Working
- Authentication
  - Working authentication for cracked clients (Origin fetching not setup yet), Creating accounts
- Class and character data saving
- Galaxy at war
- Creating games
- Matchmaking 
- Origin fetching and authentication
  
# Structure
- core *Core application structures and shared state*
- database *All application database logic and structures*
- servers *Individual server applications*
  - http *HTTP Server*
  - main *Main app server*
  - redirector *Redirector server*
- utils *Utilities used throughout the servers and core*


## EA / BioWare Notice
All code in this repository is authored by Jacobtread and none is taken from BioWare. This code has been 
produced from studying the protocol of the official servers and emulating its functionality. This program is in no way or form supported, endorsed, or provided by BioWare or Electronic Arts.