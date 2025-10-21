# MouseDrive V0.1-Alpha

This directory contains the alpha build of **MouseDrive**, a Windows console utility that maps mouse movement and button events onto a virtual joystick exposed by the vJoy driver. The goal is to give racing simulators and other titles that only recognise joystick input a way to interpret mouse steering, throttle, and braking.

## Feature Highlights
- Relative mouse X movement becomes steering on the joystick X axis (middle mouse recentres instantly).
- Left and right mouse buttons drive throttle and brake curves with configurable ramp, drop, and trail behaviour.
- W/S keyboard keys are forwarded to joystick buttons for quick gear-style inputs.
- Raw Input capture with delta clamping avoids cursor lock issues while filtering large spikes.
- vJoy integration occurs through dynamic loading, so the executable stays portable.

## Requirements
Read [REQUIREMENTS.md](REQUIREMENTS.md) for the complete list of driver and toolchain prerequisites needed to compile and run this alpha build.

## Build Instructions
1. Install the dependencies noted in the requirements document (Visual Studio with Desktop C++, Windows SDK, vJoy driver and SDK).
2. Open `MouseDrive V0.1-Alpha` as a project folder in Visual Studio or work from a Developer Command Prompt in this directory.
3. Build the single translation unit `MouseDrive.cpp`. The file already pulls in `Comctl32.lib` via `#pragma comment` and loads `vJoyInterface.dll` at runtime.
4. Compile with C++17 support. Both `Debug` and `Release` configurations produce a console application.

### Command-line Example
```powershell
cl /std:c++17 /EHsc "MouseDrive.cpp" /link comctl32.lib
```
Ensure `vJoyInterface.dll` is discoverable on the system path or next to the resulting executable before running the binary.

## Usage
1. Configure a vJoy virtual device with X, Y, and Rz axes enabled plus at least two buttons.
2. Launch `MouseDrive.exe`. The console prints the control scheme and remains active while the loop is running.
3. Drive using the mouse and buttons:
   - Mouse moves steer; middle click snaps the wheel back to centre.
   - Left mouse button applies throttle with exponential shaping tied to steering angle.
   - Right mouse button engages progressive braking with hold and release tuning.
   - `W` and `S` keys toggle joystick buttons 1 and 2 (e.g., for gears).
4. Bind your game to the vJoy device axes and buttons instead of the physical mouse.

## Configuration Overview
All runtime tuning lives in the `Config` struct inside `MouseDrive.cpp`. Parameters cover mouse sensitivity, throttle/brake curves, and timing for hold/release phases. Rebuild after editing the struct to bake new defaults into the executable.

## Directory Layout
```
MouseDrive/
├─ MouseDrive.cpp      # Single translation unit for the alpha console build.
├─ README.md           # This document.
├─ ROADMAP.md          # Planned development milestones.
└─ REQUIREMENTS.md     # Dependency checklist and build tooling notes.
```

## Contributing
- File issues with reproduction steps, OS version, and vJoy configuration details.
- Fork and submit pull requests that keep changes focused; mention which roadmap item you are targeting.
- Keep documentation in sync with the behaviour of `MouseDrive.cpp` when you add or adjust features.

## Roadmap
Refer to [ROADMAP.md](ROADMAP.md) for the planned evolution of this alpha build.

## 📝 License

Copyright © 2025 [Toxpox](https://github.com/Toxpox).<br/>
This project is [MIT License](https://github.com/Toxpox/MouseDrive/blob/main/LICENSE) licensed.

***
