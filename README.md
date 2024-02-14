<h1 align="center">
  <img src="https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/icon.svg" alt="InputPlumber Logo" width="200">
  <br>
  InputPlumber
</h1>

<p align="center">
  <a href="https://github.com/ShadowBlip/InputPlumber/stargazers"><img src="https://img.shields.io/github/stars/ShadowBlip/InputPlumber" /></a>
  <a href="https://github.com/ShadowBlip/InputPlumber/blob/main/LICENSE"><img src="https://img.shields.io/github/license/ShadowBlip/InputPlumber" /></a>
  <a href="https://discord.gg/fKsUbrt"><img src="https://img.shields.io/badge/discord-server-%235865F2" /></a>
  <br>
</p>

## About

InputPlumber is an open source input routing and control daemon for Linux. It can 
be used to combine any number of input devices (like gamepads, mice, and keyboards) 
and translate their input to a variety of virtual device formats.

Features

[x] Combine multiple input devices
[x] Emulate mouse, keyboard, and gamepad inputs
[x] Intercept and route input over DBus for overlay interface control
[ ] Input mapping profiles to translate source input into the desired target input
[ ] Route input over the network

## Install

You can install with:

```bash
make build
sudo make install
```

If you are using ArchLinux, you can install InputPlumber from the AUR:

```bash
yay -S inputplumber-bin
```

Then start the service with:

```bash
sudo systemctl enable inputplumber
sudo systemctl start inputplumber
```

## Documentation

XML specifications for all interfaces can be found in [bindings/dbus-xml](./bindings/dbus-xml).

## Usage

When InputPlumber is running as a service, you can interact with it over DBus.
There are various DBus libraries available for popular programming languages
like Python, Rust, C++, etc.

You can also interface with DBus using the `busctl` command:

```bash
busctl tree org.shadowblip.InputPlumber
```

## License

InputPlumber is licensed under THE GNU GPLv3+. See LICENSE for details.
