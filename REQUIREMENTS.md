# Requirements

This alpha build targets Windows platforms and relies on the vJoy virtual joystick driver. Install the components below before building or running `MouseDrive.cpp`.

## Operating System
- Windows 10 and later versions.

## Runtime Dependencies
This project requires [vJoy](https://github.com/jshafer817/vJoy) to be installed on the system.
vJoy is licensed under the MIT License.

- **vJoy Driver** (v2.1+): provides the virtual joystick interface (`vJoyInterface.dll`).
  - Install from [https://github.com/jshafer817/vJoy/](https://github.com/jshafer817/vJoy/). ()
  - Ensure the driver is enabled and a virtual device is configured with X, Y, and Rz axes.
- **Microsoft Visual C++ Redistributable** matching your build (if distributing binaries).
- `Comctl32.dll` (ships with Windows; needed for common controls).

## Development Toolchain
- **Microsoft Visual Studio 2019 or later** (Desktop development with C++ workload).
- **Windows 10 SDK** (10.0.17763+). Earlier SDKs work as long as they provide the Raw Input and Common Controls headers.
- C++17-compliant compiler.
- Access to the vJoy SDK headers if you intend to compile against them statically (MouseDrive loads the DLL dynamically but assumes the SDK is installed).

## Build Notes
- The project links against `Comctl32.lib` via `#pragma comment(lib, "Comctl32.lib")` inside `MouseDrive.cpp`.
- Place `vJoyInterface.dll` in the executable directory.
- Build as a console subsystem application; `MouseDrive.cpp` does not create a GUI window beyond the hidden Raw Input sink.

## Optional Tools
- **CMake** (if you prefer to create a CMakeLists.txt for cross-IDE builds).
- **vJoy Monitoring Tools** for debugging HID outputs.
