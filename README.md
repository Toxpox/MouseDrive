# MouseDrive (Rust) — v0.1.0-alpha

[![Platform](https://img.shields.io/badge/platform-Windows-0078D6)](#requirements)
[![Language](https://img.shields.io/badge/language-Rust-black)](#build)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)

MouseDrive is a Windows app that converts mouse input into a vJoy virtual joystick.

[Old C++ verison](https://github.com/Toxpox/MouseDrive-old-cpp)

## What it does

| Input | Output (vJoy) |
|------|----------------|
| Mouse X movement | X Axis (steering) |
| Left mouse button (held) | Y Axis = 100% (throttle) |
| Right mouse button (held) | Rz Axis = 100% (brake) |

## Quick start

1) Install vJoy and create a device with **X / Y / Rz** axes enabled (Device 1).

2) Make sure `vJoyInterface.dll` is accessible:
- put it next to the built executable, or
- add it to your `PATH`.

## How it works

```text
Raw Input (hidden window)  --->  Atomic globals  --->  main loop  --->  vJoy SetAxis()
```


## Requirements
- Windows 10/11
- Rust (stable) + Cargo
- vJoy Driver
- `vJoyInterface.dll` available (next to exe or in `PATH`)

## Build

Debug:

```powershell
cargo build
```

## Usage
- Start the app and keep it running.
- Bind your game to the vJoy device axes instead of mouse axes.
- Exit with `Ctrl+C`.

## Troubleshooting
- “vJoyInterface.dll not found”: put the DLL next to the exe or add it to `PATH`.
- “vJoy not enabled”: check that vJoy driver is installed and running.

## Project layout
```
MouseDrive Rust/
├─ Cargo.toml
├─ Cargo.lock
├─ src/
│  └─ main.rs
└─ README.md
```

## License

Copyright © 2025 [Toxpox](https://github.com/Toxpox).<br/>
This project is [MIT License](https://github.com/Toxpox/MouseDrive/blob/main/LICENSE) licensed.
