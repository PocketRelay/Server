<img src="https://raw.githubusercontent.com/PocketRelay/.github/main/assets/logo-new-text.svg" width="100%" height="120px">

# Pocket Relay 

*Mass Effect 3 Server Emulator / Private Server*


![License](https://img.shields.io/github/license/PocketRelay/Server?style=for-the-badge)
![Build](https://img.shields.io/github/actions/workflow/status/PocketRelay/Server/rust.yml?style=for-the-badge)

[Discord Server (discord.gg/yvycWW8RgR)](https://discord.gg/yvycWW8RgR)
[Website (pocket-relay.pages.dev)](https://pocket-relay.pages.dev/)

Development is undergone on the [dev](https://github.com/PocketRelay/Server/tree/dev) branch so the master branch can be
considered semi stable with only non breaking changes being merged in-between releases

**Pocket Relay** Is a custom implementation of the Mass Effect 3 multiplayer servers all bundled into a easy to use server with a Dashboard for managing accounts and inventories.

With **Pocket Relay** you can play Mass Effect 3 multiplayer offline by yourself, over LAN, or even over WAN as a public server 

View the website for information https://pocket-relay.pages.dev/


## 📌 EA / BioWare Notice

The **Pocket Relay** software in all its forms are in no way or form supported, endorsed, or provided by BioWare or Electronic Arts. Mass Effect is a registered trademark of Bioware/EA International (Studio and Publishing), Ltd in the U.S. and/or other countries. All Mass Effect art, images, and lore are the sole property of Bioware/EA International (Studio and Publishing), Ltd and have been reproduced here in an effort to assist the Mass Effect player community. All other trademarks are the property of their respective owners.


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

In order to configure the server such as changing the ports you can see the
configuration documentation [Here (docs/CONFIG.md)](https://pocket-relay.pages.dev/guide/config/)


## ⚙️ Features

- **Origin Support** This server supports **Origin** / **EA Launcher** copies of the game through its fetching system. As long as the official servers are still available and you have internet access the server will connect to the official servers to authorize **Origin** accounts. *This behavior can be disabled in the configuration*
- **Origin Fetching** Along with supporting **Origin** authentication your player data from the official servers can also be loaded for those logging into **Origin** accounts. *This behavior can be disabled in the configuration*
- **Portable & Platform Independent** This server can be run on most hardware and software due to its low requirements and custom
implementations of lots of required portions allowing you to run it
on Windows, Linux, etc. *Note the server will store the player data and logging in a folder named `data` in the same folder as the exe*
- **Unofficial Support** This server unofficially licensed Mass Effect 3 copies so you can play on the server using them.
- **Docker Support** This server includes a `Dockerfile` so that it can be run in a containerized environment. The server uses a small alpine linux container to run inside
- **Dashboard** The server includes a management dashboard 
    - This includes leaderboards displays
    - Allowing players to edit their username, email, and password
    - Deleting and managing accounts
    - Viewing running games
    - Inventory editing for admins (Weapons, Classes, Characters, etc)
    - View server logs as super admin

## 🚀 Manual Building

Instructions for building the server can be found at https://pocket-relay.pages.dev/docs/server/manual-building

> **Note**
> Building the server can be quite a heavy load on your computer


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