# Changelog

All notable changes to MouseDrive are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-06-14

The headline of this release is the **decoupled control architecture**: the
250 Hz vJoy feed now runs on its own thread, completely independent of the GUI.

### Performance & architecture

- **Control loop decoupled from the GUI thread** — `control.rs` owns the vJoy
  handle and runs a 250 Hz loop on a dedicated `THREAD_PRIORITY_HIGHEST` thread.
  The vJoy feed no longer stalls when the window is minimized or the repaint
  lags — previously the steering axis froze and the accumulated mouse delta was
  applied as one jump on restore. The GUI now only reads a shared lock-free
  snapshot and publishes config changes; control math is byte-identical (all 41
  logic tests unchanged).
- **Lazy GUI repaint** — ~60 Hz focused, ~4 Hz backgrounded. Control cadence is
  fully independent of repaint rate.
- **`panic = "abort"`** in the release profile (smaller binary, no unwind tables).
- Configurable **vJoy device id** (1–16) in Settings > General — no longer stuck
  on DeviceBusy when Device 1 is in use.
- **Lightweight file logging** (`mousedrive.log` next to the config) for
  connect/reconnect outcomes, missing DLL/symbol, and update failures; never on
  the hot path. `VJoyApi` relinquishes the device on `Drop` as a shutdown backstop.
- **`exit_on_close` now works** — unchecked → minimize (vJoy feed keeps running);
  default stays "exit".
- Config validation **surfaces the corrected-field count** in the UI; added a
  `config_version` migration hook for future schema changes.
- **Updater is an optional `updater` cargo feature** (default on).
  `--no-default-features` drops ureq/rustls/zip/sha2/self-replace → lean build
  (~3.9 MB vs ~5.3 MB).

### UI / visual

- **Modern dashboard redesign**: accent blue theme (`#378ADD`), vertical
  colour-coded gauges (throttle green / brake red), bidirectional steering bar
  (custom painter, centre-fill), status and input pill badges.
- **Readability fix**: selected tabs / combo-box items now show white text on blue
  background. Previously `selection.stroke.color = ACCENT` made text invisible
  (blue-on-blue).
- Localization: added `steer_left` / `steer_right` (TR "Sol/Sağ", EN "Left/Right").

### Added

- **Graphical envelope curve editors** for throttle rise/fall, brake apply, and
  brake post-hold drop. Drag control points (2–8) directly on the graph,
  double-click to add, right-click to remove. Two interpolation modes: Linear
  and Smooth (monotone cubic / PCHIP — guaranteed no overshoot). Presets:
  Linear, S-Curve, Aggressive, Progressive. A live marker travels along the
  active curve while driving. (Design notes: `graph.md`)
- **Phase-tracking ramp algorithm**: throttle/brake envelopes follow the curves
  with inverse re-seeding on every direction change, so output is continuous
  across press/release/steering-cut transitions. Default identity curves
  reproduce the previous linear behavior exactly — old configs feel identical.
- **Automatic update checks**: once per day on startup (configurable), in a
  background thread that never blocks the control loop. Manual "Check now"
  button in Settings > General.
- **One-click self-update**: a green Update button appears in the top bar when
  a new release is available. Clicking it downloads the release zip, verifies
  it against `SHA256SUMS.txt`, swaps the running executable and restarts
  automatically. Falls back to opening the release page if the release lacks
  standardized assets or installation fails. "Skip" silences a given version.
  (Design notes: `auto-update.md`)
- **CI/CD pipeline** (`.github/workflows/ci.yml`): tests + clippy on every
  push/PR; pushing a `vX.Y.Z` tag builds with the version taken from the tag,
  packages `MouseDrive-vX.Y.Z-windows-x64.zip` + `SHA256SUMS.txt`, and
  publishes a GitHub release automatically.
- New config options: `auto_check_updates`, `skipped_version`, and per-curve
  tables (`throttle_rise_curve`, `throttle_fall_curve`, `brake_apply_curve`,
  `brake_posthold_curve`). Existing `config.toml` files load unchanged.
- `.gitignore` and `CHANGELOG.md`.

### Changed

- Throttle and brake ramp steps are now evaluated through envelope curves; the
  existing `*_ms` sliders remain the time base, curves only shape the profile.
- The brake post-hold falloff curve composes on top of the existing
  "Release Accel Power" exponent (set the exponent to 1.0 for purely graphical
  control). Button-release decay intentionally stays linear.
- Test suite grew from 16 to 41 unit tests (curve math, monotonicity, inverse
  round-trips, legacy-equivalence regression, config compatibility, update
  parsing/verification).

### Fixed

- `main` failed to compile: `Win32_Media` and `Win32_System_Threading` features
  were missing from `Cargo.toml` (regression relative to the v0.4.0 release).
- `Cargo.toml` package version (stuck at 0.3.0) brought in line with the actual
  release line; from now on CI stamps the version from the git tag at build time.

## [0.4.0] - 2026-03-21

- Steering, throttle and brake tuning improvements; UI polish.
- See the [v0.4.0 release](https://github.com/Toxpox/MouseDrive/releases/tag/v0.4.0).

## [0.3.0] - 2026-02-09

- Rust rewrite of the original C++ version: raw-input capture thread, atomic
  lock-free state sharing, eframe/egui interface, TOML configuration, TR/EN
  localization.
- See the [V0.3.0 release](https://github.com/Toxpox/MouseDrive/releases/tag/V0.3.0).

## [0.1.0-alpha] - 2026-02-04

- First public alpha.
- See the [V0.1.0-alpha release](https://github.com/Toxpox/MouseDrive/releases/tag/V0.1.0-alpha).

[0.5.0]: https://github.com/Toxpox/MouseDrive/compare/v0.4.0...main
[0.4.0]: https://github.com/Toxpox/MouseDrive/compare/V0.3.0...v0.4.0
[0.3.0]: https://github.com/Toxpox/MouseDrive/compare/V0.1.0-alpha...V0.3.0
[0.1.0-alpha]: https://github.com/Toxpox/MouseDrive/releases/tag/V0.1.0-alpha
