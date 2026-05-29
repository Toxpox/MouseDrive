# MouseDrive (Rust) — Beta Branch

[![Language](https://img.shields.io/badge/language-Rust-black)](#build)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2FToxpox%2FMouseDrive.svg?type=shield)](https://app.fossa.com/projects/git%2Bgithub.com%2FToxpox%2FMouseDrive?ref=badge_shield)

MouseDrive is a Windows application that converts mouse and keyboard input into virtual joystick signals via [vJoy](https://github.com/BrunnerInnovation/vJoy), designed for racing simulators.

[Old C++ version](https://github.com/Toxpox/MouseDrive-old-cpp)

## Download

**[Download latest release](https://github.com/Toxpox/MouseDrive/releases/latest)**

Or browse all versions at [Releases](https://github.com/Toxpox/MouseDrive/releases/).

> Extract the `.zip`, place `vJoyInterface.dll` next to `mousedrive.exe`, and run.


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

## Requirements

- Windows 10/11
- [Executable MouseDrive](https://github.com/Toxpox/MouseDrive/releases/latest)
- [vJoy Driver**](https://github.com/BrunnerInnovation/vJoy) installed and enabled
- `vJoyInterface.dll` available (next to exe, Program Files, or in `PATH`)

>  ** Tested with V2.2.2.0

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "vJoyInterface.dll not found" | Place the DLL next to the exe, in `Program Files\Shaul\vJoy\x64`, or add its folder to `PATH` |
| "vJoy not enabled" | Check that vJoy driver is installed and the service is running |
| "vJoy device busy" | Another application is using Device 1 — close it or use a different device |
| Mouse not captured | Press **F8** to toggle input capture |
| Settings not saving | Check write permissions in `%APPDATA%\MouseDrive\` |

## License

Copyright (c) 2025-2026 [Toxpox](https://github.com/Toxpox).
This project is licensed under the [MIT License](https://github.com/Toxpox/MouseDrive/blob/main/LICENSE).

[![FOSSA Status](https://app.fossa.com/api/projects/git%2Bgithub.com%2FToxpox%2FMouseDrive.svg?type=large)](https://app.fossa.com/projects/git%2Bgithub.com%2FToxpox%2FMouseDrive?ref=badge_large)
