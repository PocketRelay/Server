<img src="https://raw.githubusercontent.com/PocketRelay/.github/main/assets/logo-new-text.svg" width="100%" height="160px">

# Pocket Relay 

> Mass Effect 3 Server Emulator Rust server implementation

![License](https://img.shields.io/github/license/PocketRelay/ServerRust?style=for-the-badge)
![Cargo Version](https://img.shields.io/crates/v/pocket-relay?style=for-the-badge)
![Cargo Downloads](https://img.shields.io/crates/d/pocket-relay?style=for-the-badge)

This implementation of Pocket Relay is still under active development. See the [Kotlin Server (https://github.com/PocketRelay/ServerKotlin)](https://github.com/PocketRelay/ServerKotlin)
for a working usable implementation

Development on this implementation has made great strides and has surpassed the original Kotlin server in terms of performance
reliabality, functionality and actual parody to the official server. However as this implementation is still considered to be
in active development so no releases will be published YET. However you are welcome to build it yourself.

Because of this you may want to use the [Kotlin Server (https://github.com/PocketRelay/ServerKotlin)](https://github.com/PocketRelay/ServerKotlin) for the time being until this server is available for use.

# ❔ What

This is an implementation of a Mass Effect 3 multiplayer server. This server emulates the functionality of the official servers except it can run locally on your machine and be shared over LAN allowing you to play Mass Effect 3 multiplayer 
without an internet connection or just locally for a LAN party.

# 🔌 Client Usage
In order to connect to a Pocket Relay server you will first need to use the Client tool to patch your
game and update your hosts file with the address of the server.

You can find the client [Here (https://github.com/PocketRelay/Client)](https://github.com/PocketRelay/Client)

The client tool makes it easy to do the setup process. But its possible to do everything the client
does manually if you'd prefer. Documentation for how to do so will be posted at a later point.

# 📦 Releases

As this implementation is still under active development there will be no 
releases until its closer to being considered user ready. However you can
still use this server early by building manually with the instructions below

# 🚀 Building

> Note: There is a Dockerfile included in the root of this repository which can
> automatically build and run the server if you're willing to run it within Docker

## 📄 Building Requirements

In order to build Pocket relay you must have the following tools installed

- Rust v1.65 or greater with the stable channel installed. ([You can find it here](https://www.rust-lang.org/learn/get-started))
- Cargo (Should be included with your Rust installation if you installed it through rustup)

To build the release binary run the following command:

```shell
cargo build --release
```

After the compiling is complete you will find the server binary at
```
target/release/pocket-relay.exe
```

The produced version will use the SQLite database type if you would instead like
to build a version of Pocket Relay that supports MySQL databases instead
you can use the following command

```shell
cargo build --release --features database-mysql --no-default-features
```

The `--no-default-features` flag disables the default SQLite database features if you would
like the executable to support both MySQL and SQLite you can omit this field however including both database types will greatly increase the binary size.

# ⚙️ Features

**Origin Authentication** This server supports origin authentication through the official servers. This is enabled by default but can be changed using the PR_ORIGIN_FETCH environment variable.

> NOTE: Origin Authentication requires that the PR_RETRIEVER must not be false. If the 
> retriever system is disable then it won't be allowed to connect to the official servers

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
  
# 📂 Structure
- 📁 core *Core application structures and shared state*
- 📁 database *All application database logic and structures*
- 📁 servers *Individual server applications*
  - 📁 http *HTTP Server*
  - 📁 main *Main app server*
  - 📁 redirector *Redirector server*
- 📁 utils *Utilities used throughout the servers and core*


# 📌 EA / BioWare Notice
All code in this repository is authored by Jacobtread and none is taken from BioWare. This code has been 
produced from studying the protocol of the official servers and emulating its functionality. This program is in no way or form supported, endorsed, or provided by BioWare or Electronic Arts.

# 🧾 License

MIT License

Copyright (c) 2022 Jacobtread

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