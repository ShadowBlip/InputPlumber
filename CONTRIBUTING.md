# Contributing

Contributing to InputPlumber is done via Pull Requests on the main github repository: [InputPlumber](https://github.com/shadowblip/InputPlumber).

## Commit Titles

This project uses [semantic-release](https://github.com/semantic-release/semantic-release) which uses Angular-style commit messages in order to automatically trigger releases.

You can read more about it [here](https://github.com/angular/angular/blob/main/CONTRIBUTING.md#-commit-message-format).

In short every commit must be in the form of:

- chore(*thing*): Title words
- fix(*thing*): Title words
- feat(*thing*): Title words
- docs(*thing*): Title words

This is a non-exhaustive list, please read the above link to learn more.

## General Rules

Files related to IDEs are not allowed in the repository.

Every commit must go through

```sh
cargo fmt
```

so make sure to run it before any new commit.

## Testing and debugging

In order to run the application you have to stop every other running instances and launch as the in-development version as root:
this can be achieved launching lldb-server as root and connecting to that service to debug InputPlumber.

The command is the following:

```sh
lldb-server platform --listen "*:1234" --server
```

if you want to use a systemd service:

```ini
[Unit]
Description=lldb debug server
Wants=network-online.target
After=network.target network-online.target

[Service]
ExecStart=lldb-server platform --listen "*:1234" --server
Restart=always
WorkingDirectory=/var/lldb-debug-server

[Install]
WantedBy=multi-user.target
```

however remember to disable it when you are done and to be on a secure network as no password will be asked and every executable sent
will be run as root.

A minimal configuration for VSCode with CodeLLDB extension to match the previous command is as follows:

```json
{
    
    "version": "0.2.0",
    "configurations": [
        {
            "name": "Remote launch",
            "type": "lldb",
            "request": "launch",
            "program": "${workspaceFolder}/target/debug/inputplumber",
            "initCommands": [
                "platform select remote-linux",
                "platform connect connect://192.168.1.19:1234",
                "settings set target.inherit-env false"
            ],
            "env": {
                "PATH": "/usr/bin:/usr/local/sbin:/usr/local/bin:/var/lib/flatpak/exports/bin:/usr/lib/rustup/bin"
            }
        }
    ]
}
```
