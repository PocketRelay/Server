# Pocket Relay
 
<img src="https://raw.githubusercontent.com/PocketRelay/.github/main/assets/logo-new-text.svg" width="100%" height="120px">


*Mass Effect 3 Server Emulator / Private Server*

![License](https://img.shields.io/github/license/PocketRelay/Server?style=for-the-badge)
![Build](https://img.shields.io/github/actions/workflow/status/PocketRelay/Server/rust.yml?style=for-the-badge)


[Discord Server (discord.gg/yvycWW8RgR)](https://discord.gg/yvycWW8RgR) | [Website (pocket-relay.pages.dev)](https://pocket-relay.pages.dev/)


The master branch contains the latest changes and may not be stable for general use, if you would like to compile a stable version from source its recommended you use a specific tag rather than master

**Pocket Relay** Is a custom implementation of the Mass Effect 3 multiplayer servers all bundled into a easy to use server with a Dashboard for managing accounts and inventories.

With **Pocket Relay**, you can play Mass Effect 3 multiplayer offline, over LAN, or even over WAN as a public server.

Visit the [website](https://pocket-relay.pages.dev/) for more information.


## 🌐 EA / BioWare Notice

The **Pocket Relay** software, in all its forms, is not supported, endorsed, or provided by BioWare or Electronic Arts. Mass Effect is a registered trademark of Bioware/EA International (Studio and Publishing), Ltd in the U.S. and/or other countries. All Mass Effect art, images, and lore are the sole property of Bioware/EA International (Studio and Publishing), Ltd and are reproduced here to assist the Mass Effect player community. All other trademarks are the property of their respective owners.


## 📖 Starting your own server

For guides check out the [Website (pocket-relay.pages.dev)](https://pocket-relay.pages.dev/) or refer directly to 
the [Server Setup Guide](https://pocket-relay.pages.dev/guide/server/)

## 📦 Direct Downloads

Below is a table of the download links for the different platforms

| Platform     | Download                                                                                                        |
| ------------ | --------------------------------------------------------------------------------------------------------------- |
| Windows      | [Download](https://github.com/PocketRelay/Server/releases/latest/download/pocket-relay-x86_64-windows-msvc.exe) |
| Linux        | [Download](https://github.com/PocketRelay/Server/releases/latest/download/pocket-relay-x86_64-linux-musl)       |
| Linux  (ARM) | [Download](https://github.com/PocketRelay/Server/releases/latest/download/pocket-relay-aarch64-linux-musl)       |

You can find individual releases on the [Releases](https://github.com/PocketRelay/Server/releases) page


## 🔧 Configuration

To configure the server, such as changing ports, refer to the [Configuration Documentation](https://pocket-relay.pages.dev/guide/config/).

## ⚙️ Features

- **Origin Support:** Connects to official servers to authorize **Origin**/**EA Launcher** accounts (configurable).
- **Origin Fetching:** Loads player data from official servers for **Origin** accounts (configurable).
- **Portable & Platform Independent:** Low hardware requirements, platform-independent (data stored in a 'data' folder).
- **Unofficial Support:** Allows playing with unofficially licensed Mass Effect 3 copies.
- **Docker Support:** Includes a `Dockerfile` for containerized deployment in a small Alpine Linux container.
- **Dashboard:** Management dashboard with leaderboards, account management, game monitoring, and more.


## 🚀 Manual Building

Build instructions can be found [here](https://pocket-relay.pages.dev/docs/server/manual-building).

> **Note**
> Building the server can be resource-intensive.
>
> If you are building for a version older than Windows 10 you will need to use Rust v1.75.0 or lower as 
> Rust has dropped support for <10 after that. The server should compile on this version but future breaking
> changes to the project may cause that to no longer be the case.

## Makefile.toml - Mainly used for maintainers 

This project also includes a Makefile.toml for `cargo make` however its more intended for maintainers only in order to do cross compiling, building multiple versions in parallel, signing builds, creating docker releases etc

> Requires installing https://github.com/sagiegurari/cargo-make

### Building

#### Build Windows & Linux in parallel

```shell
cargo make -t build-all
```
#### Building just Windows

```shell
cargo make -t build-windows
```

> [!NOTE]
> When building for Windows on a Windows host you can sign the executable by providing a `SIGN_FILE` (File path to the .pfx file to use for signing) and `SIGN_PASSWORD` (The password to the .pdf file) you will also need to obtain a copy of signtool.exe and set the `SIGNTOOL_PATH` to be the path to that file
>
> After doing that Windows builds will be signed using the provided credentials

#### Building just Linux

```shell
cargo make -t build-linux
```

### Docker images

> [!IMPORTANT]
> The intended release must first be up on GitHub once its up on GitHub make sure to set the `GITHUB_RELEASE_VERSION` environment variable so the right version will be used make sure you don't include the v prefix just the version number (e.g 0.6.1)
>
> The `DOCKER_IMAGE_NAME` env variable must be set to the intended docker image name (e.g jacobtread/pocket-relay)

#### Building the docker image

To build for the specific tag (Uses the version listed in the Cargo.toml):

```shell
cargo make -t build-docker-version
```

To build for the "latest" tag:

```shell
cargo make -t build-docker-latest
```

To build for both tags:

```shell
cargo make -t build-docker-all
```

### Publishing docker images

To publish for the specific tag (Uses the version listed in the Cargo.toml):

```shell
cargo make -t publish-docker-version
```

To publish for the "latest" tag:

```shell
cargo make -t publish-docker-latest
```

To publish for both tags:

```shell
cargo make -t publish-docker-all
```
