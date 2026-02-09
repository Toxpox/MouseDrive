# MouseDrive (Rust) — v0.3.0

[![Platform](https://img.shields.io/badge/platform-Windows-0078D6)](#requirements)
[![Language](https://img.shields.io/badge/language-Rust-black)](#build)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2FToxpox%2FMouseDrive.svg?type=shield)](https://app.fossa.com/projects/git%2Bgithub.com%2FToxpox%2FMouseDrive?ref=badge_shield)

MouseDrive is a Windows application that converts mouse and keyboard input into virtual joystick signals via [vJoy](https://github.com/BrunnerInnovation/vJoy), designed for racing simulators.

[Old C++ version](https://github.com/Toxpox/MouseDrive-old-cpp)

## Download

**[Download latest release](https://github.com/Toxpox/MouseDrive/releases/latest)**

Or browse all versions at [Releases](https://github.com/Toxpox/MouseDrive/releases/).

> Extract the `.zip`, place `vJoyInterface.dll` next to `mousedrive.exe`, and run.

## Features

- **Steering** — Mouse X movement mapped to vJoy X axis with 4 modes (linear, expo*, filtered*, self-centering)
- **Throttle** — Left mouse button with configurable ramp/drop curves
- **Brake** — Right mouse button with full state machine (press, hold, release)
- **Gear buttons** — W/S keys mapped to vJoy buttons 1/2
- **Live dashboard** — Real-time progress bars for steering, throttle, brake
- **Tabbed settings panel** — Collapsible side panel with per-category tuning
- **Config persistence** — TOML-based config with load/save/reset
- **Raw input capture** — Works even when the window is not focused (F8 toggle)
- **Lock-free architecture** — Atomic globals between raw input thread and GUI thread
- **Language support** — Turkish and English UI (switchable in Settings > General)

> *Experimental

## Input / Output mapping

| Input | vJoy Output | Control |
|-------|-------------|---------|
| Mouse X movement | X Axis | Steering |
| Left mouse button (held) | Y Axis | Throttle |
| Right mouse button (held) | Rz Axis | Brake |
| W key | Button 1 | Gear up |
| S key | Button 2 | Gear down |
| Middle click | — | Reset steering |
| F8 | — | Toggle input capture |

## Quick start

1. Install [vJoy](https://github.com/BrunnerInnovation/vJoy) and create a device with **X / Y / Rz** axes and **2 buttons** enabled (Device 1).
2. Make sure `vJoyInterface.dll` is accessible (next to the executable or in your `PATH`).
3. Run [Executable MouseDrive](https://github.com/Toxpox/MouseDrive/releases/latest).
4. Bind your game to the vJoy device axes.

## How it works

- **Raw input thread** — Dedicated thread with a hidden window that receives `WM_INPUT` messages for mouse deltas and button states.
- **Atomic communication** — `AtomicI64`, `AtomicBool`, `AtomicU64` for zero-lock data transfer between threads.
- **GUI thread** — eframe/egui loop that reads atomics, runs control logic, updates vJoy, and renders the UI.

## Requirements

- Windows 10/11
- [Executable MouseDrive](https://github.com/Toxpox/MouseDrive/releases/latest)
- [vJoy Driver**](https://github.com/BrunnerInnovation/vJoy) installed and enabled
- `vJoyInterface.dll` available (next to exe or in `PATH`)

>  ** Tested with V2.2.2.0

## Build

```powershell
cargo build

cargo build --release
```


## Configuration

Settings are stored in TOML format. The config file is loaded from:

1. `<exe_directory>/config.toml` (portable mode)
2. `%APPDATA%\MouseDrive\config.toml` (standard)

Use the **Save** / **Load** / **Default** buttons in the settings panel, or edit the file manually.

## Project layout

```
MouseDrive/
├── src/
│   ├── main.rs      # App struct, entry point, vJoy connection
│   ├── config.rs    # Config struct, TOML load/save, path resolution
│   ├── input.rs     # Raw input thread, atomic globals, key helpers
│   ├── logic.rs     # Steering/throttle/brake state machine & math
│   ├── ui.rs        # egui UI (side panel, tabs, dashboard)
│   └── vjoy.rs      # vJoy FFI (runtime DLL loading, axis/button API)
├── Cargo.toml
├── LICENSE
└── README.md
```

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "vJoyInterface.dll not found" | Place the DLL next to the exe or add its folder to `PATH` |
| "vJoy not enabled" | Check that vJoy driver is installed and the service is running |
| "vJoy device busy" | Another application is using Device 1 — close it or use a different device |
| Mouse not captured | Press **F8** to toggle input capture |
| Settings not saving | Check write permissions in `%APPDATA%\MouseDrive\` |

## License

Copyright (c) 2025-2026 [Toxpox](https://github.com/Toxpox).
This project is licensed under the [MIT License](https://github.com/Toxpox/MouseDrive/blob/main/LICENSE).

[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2FToxpox%2FMouseDrive.svg?type=large)](https://app.fossa.com/projects/git%2Bgithub.com%2FToxpox%2FMouseDrive?ref=badge_large)
