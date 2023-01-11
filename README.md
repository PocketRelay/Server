<img src="https://raw.githubusercontent.com/PocketRelay/.github/main/assets/logo-new-text.svg" width="100%" height="160px">

# Pocket Relay 

> Mass Effect 3 Server Emulator Rust server implementation

![License](https://img.shields.io/github/license/PocketRelay/ServerRust?style=for-the-badge)
![Cargo Version](https://img.shields.io/crates/v/pocket-relay?style=for-the-badge)
![Cargo Downloads](https://img.shields.io/crates/d/pocket-relay?style=for-the-badge)
![Build](https://img.shields.io/github/actions/workflow/status/PocketRelay/ServerRust/rust.yml?style=for-the-badge)

Development on this implementation has made great strides and has surpassed the original Kotlin server in terms of performance
reliabality, functionality and actual parody to the official server. However as this implementation is still considered to be
in active development.

# 📖 Setting up a server

For a guide on how to setup a Pocket Relay server see [Here](https://github.com/PocketRelay/.github/blob/main/manual/SETUP_SERVER.md)
and for connecting to a server see [Here](https://github.com/PocketRelay/.github/blob/main/manual/SETUP_CLIENT.md)


# 🎮 Pre Release

You can find pre-release downloads in the [Releases (https://github.com/PocketRelay/ServerRust/releases)](https://github.com/PocketRelay/ServerRust/releases). The most recent pre release is 
mostly stable but may have unknown bugs or features that are expecting to change 

Latest pre release is available  [Here](https://github.com/PocketRelay/ServerRust/releases/latest)

# ❔ What

This is an implementation of a Mass Effect 3 multiplayer server. This server emulates the functionality of the official servers except it can run locally on your machine and be shared over LAN allowing you to play Mass Effect 3 multiplayer 
without an internet connection or just locally for a LAN party.

# 🔌 Client Usage
In order to connect to a Pocket Relay server you will first need to use the Client tool to patch your
game and update your hosts file with the address of the server.

You can find the client [Here (https://github.com/PocketRelay/Client)](https://github.com/PocketRelay/Client)

The client tool makes it easy to do the setup process. But its possible to do everything the client
does manually if you'd prefer. Documentation for how to do so will be posted at a later point.

# 🔧 Configuration

In order to configure the server such as changing the ports you can see the
configuration documentation [Here (docs/CONFIG.md)](docs/CONFIG.md)

# 📦 Releases

As this implementation is still under active development there will be no 
stable releases until its closer to being considered user ready. However you can
still use this server early by building manually with the instructions below

The latest available release can be found [Here](https://github.com/PocketRelay/ServerRust/releases/latest)

# 🚀 Building

Instructions for building the server can be found [Here](https://github.com/PocketRelay/.github/blob/main/manual/BUILDING.md)

# ⚙️ Features

**Origin Authentication** This server supports origin authentication through the official servers. This is enabled by default but can be changed using the PR_ORIGIN_FETCH environment variable.

> NOTE: Origin Authentication requires that the PR_RETRIEVER must not be false. If the 
> retriever system is disabled then it won't be allowed to connect to the official servers

**Origin Fetching** This server supports copying over Origin credits, classes, characters, weapons, levels, etc from the official servers. This is enabled by default and can
be changed using the PR_ORIGIN_FETCH_DATA environment variable.

**API** This portion of the server is currently experimental and its an API which allows 
querying the server for information such as the active Games and querying the database for
player information.

**Man-In-The-Middle Server** This server has a built in Man-In-The-Middle server for proxying your connection to the Official servers while logging all the packets that travel
between the client and the server. 

**Dockerized** This server is able to be built and run in an Alpine linux docker container 
removing the need for building and managing files and executables.

**Fully portable & Platform Independent** Other Mass Effect 3 servers were limited by the operating systems that they were able to run on due to limitations with SSLv3 which is required by a portion of the server to function. However this server has its own 
custom SSLv3 implementation and can be compiled for Windows, Linux and even Alpine linux.
The entire server fits inside a single executable and the only files it writes to the drive
are log files and the database if using the SQLite database version. 

**Cracked & Origin Support** This server supports both cracked and Origin clients.

# 📡 API

Pocket Relay servers contain an API which allows you to query the list of players and view currently active games.
This is not enabled by default but see the configuration documentation for the variable to enable it. 

You can view its documentation [Here (docs/API.md)](docs/API.md)

# ⛔️ Known Issues

**Host Migration** Host migration is partially working however it only currently works
for a single player. If there is more than one player in the game when host migration
occurs currently all the other players get booted from the game except for the new host.
This also causes the game to become un-joinable for other players.

> Host migration is a fixable issue it just needs more testing and experimenting

**Inital connection failure on menu when using cracked game** When connecting to the server
for the first time when you've originally logged into the official servers you will be given
a connection error screen. You can just press okay on this error this is because the server couldn't
authenticate you using a session token that was for the original server. Once you're on
the main menu you can push the button on the terminal or the "Multiplayer" button and you will be
taken to a login screen where you can create a new account with Pocket Relay.

> The only way to solve this issue would be to delete or move the Local_Profile.sav file
> whenever switching servers in order to clear the token.
  
# 🚧 Stablity 

The following is a list of the servers and how stable they are for production use. The table below describes each level of stablity

| Level         | Description                                                                                                       |
| ------------- | ----------------------------------------------------------------------------------------------------------------- |
| 🟢 STABLE      | The server is stable for use in production                                                                        |
| 🟡 SEMI-STABLE | The server is mostly stable but is not fully tested and may have edge cases or unknown issues. Needs more testing |
| 🟠 UNSTABLE    | The server is not stable and should not be used in production                                                     |

| Server     | Stablity | Details                                                                                                  |
| ---------- | -------- | -------------------------------------------------------------------------------------------------------- |
| Redirector | 🟢        | The redirector server is quite simple and is fully implemented and function                              |
| MITM       | 🟢        | The MITM server only needs to pass packets onto the official server so this is complete                  |
| TELEMETRY  | 🟡        | The telemetry server doesn't actual interract with anything meaningful so at this stage its semi stable  |
| QOS        | 🟢        | The QOS server is fully implemented so is consider to be stable                                          |
| HTTP       | 🟡        | The HTTP server is mostly stable and implements the features it needs but needs to be tested             |
| Main       | 🟡        | The Main server is mostly stable however changes are expected and not all functionality works correectly |


# 📌 EA / BioWare Notice

All code in this repository is authored by Jacobtread and none is taken from BioWare. This code has been 
produced from studying the protocol of the official servers and emulating its functionality. This program is in no way or form supported, endorsed, or provided by BioWare or Electronic Arts.

# 🧾 License

MIT License

Copyright (c) 2022 - 2023 Jacobtread

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.