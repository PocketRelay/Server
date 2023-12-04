# Pocket Relay
 
<img src="https://raw.githubusercontent.com/PocketRelay/.github/main/assets/logo-new-text.svg" width="100%" height="120px">


*Mass Effect 3 Server Emulator / Private Server*

![License](https://img.shields.io/github/license/PocketRelay/Server?style=for-the-badge)
![Build](https://img.shields.io/github/actions/workflow/status/PocketRelay/Server/rust.yml?style=for-the-badge)


[Discord Server (discord.gg/yvycWW8RgR)](https://discord.gg/yvycWW8RgR) | [Website (pocket-relay.pages.dev)](https://pocket-relay.pages.dev/)


Development is carried out on the [dev](https://github.com/PocketRelay/Server/tree/dev) branch, making the master branch semi-stable with only non-breaking changes merged between releases.

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

| Platform | Download                                                                                            |
| -------- | --------------------------------------------------------------------------------------------------- |
| Windows  | [Download](https://github.com/PocketRelay/Server/releases/latest/download/pocket-relay-windows.exe) |
| Linux    | [Download](https://github.com/PocketRelay/Server/releases/latest/download/pocket-relay-linux)       |

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


## 🧾 License

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