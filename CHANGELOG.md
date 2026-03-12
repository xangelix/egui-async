# Changelog

## v0.3.5

This release introduces a complete rewrite and revamp of the async widgets. It also includes documentation updates and dependency bumps.

### Added

- **Widgets (`egui_async::egui`):** A suite of battle-tested, drop-in UI components that handle loading states, layout snapping, and debouncing automatically.
  - **`AsyncButton`:** An action button that prevents double-submissions by disabling itself and showing an inline spinner while operations are pending. It calculates geometry ahead of time to ensure zero visual shifting between states.
  - **`AsyncView`:** A declarative container that completely manages the `Idle`, `Pending`, `Failed`, and `Finished` lifecycles of a data fetch. It features a configurable `StateLayout` (e.g., `CenterHorizontal`, `Inline`) to prevent jarring layout snapping during state transitions.
  - **`AsyncSearch`:** A decoupled text input widget that natively handles debounce timers and async typeahead searches, displaying results in a floating dropdown portal.

### Documentation

- **README:** Updated and reworded small examples in the `README.md` to introduce the new "Widgets" pattern and provide guidance on building custom async-aware components.

### Chore

- **Dependencies:** Bumped `dev-dependencies` to their latest versions.

## v0.3.4

### Fixed

- **README Badges:** Restored the status badges (Crates.io, Docs.rs, License, WASM support) to the top of `README.md` which were accidentally removed during the v0.3.3 documentation overhaul.

## v0.3.3

This release focuses on documentation improvements to make the library easier to learn and use, illustrating specific UI patterns for common async tasks.

### Documentation

- **README Overhaul:** Completely rewrote the `README.md` to align with a cleaner, high-performance style.
    - Introduced a **"Usage Patterns"** section covering Lazy Loading, Explicit State Machines, Periodic Refresh, and Widgets.
    - Added an **"Under the Hood"** section explaining the plugin architecture, channel polling, and task spawning differences between Native and WASM.
    - Added **"Common Pitfalls"** and **"Compatibility"** sections.
    - Added a diagram placeholder for the event loop architecture.

### Added

- **Login Example:** Added `examples/login.rs`. This is a complete, runnable example demonstrating the "Explicit State Control" pattern (Pattern 2), featuring a mock authentication flow, `StateWithData` matching, and `egui::Grid` layout.

## v0.3.2

This release focuses on the robustness of the `Bind::fill` API and significant stability improvements for the test suite in concurrent environments.

### Changed

- **`Bind::fill` Robustness:** `fill()` no longer panics if the bind is not `Idle`. Instead, it now deterministically aborts any in-flight operations and overwrites existing data. This enables "force update" patterns without requiring manual state checks.
- **Hot Loop Detection:** `fill()` now logs a warning if called multiple times in the same frame to help identify unintentional logic loops in UI updates.

### Fixed

- **Test Suite Stability:** Resolved multiple race conditions where global frame timer contention caused flaky test failures. Tests now correctly isolate `Bind` instances using `retain=true` and use deterministic future barriers.
- **Lints:** Applied fixes for `clippy::duration_suboptimal_units` and other minor static analysis warnings.

## v0.3.1

- Small bug fix for overlapping egui IDs when using the `popup_error` and `popup_notify` helpers.

## v0.3.0

This release introduces the ability to physically abort background tasks on native targets and improves the internal state synchronization of the `Bind` lifecycle. It also establishes the project's baseline CI/CD pipeline.

### Added

- **Task Abortion (Native):** Introduced `ConfigFlags::ABORT` (and `Bind::set_abort`). When enabled on native targets, `Bind` will physically terminate background Tokio tasks when they are cleared or replaced by a new request.
- **CI/CD:** Initialized GitHub Actions for automated testing and `cargo fmt` verification.

### Changed

- **Internal Architecture:** Refactored task management from a raw `oneshot::Receiver` to a structured `InFlight` container. This ensures that the task handle and the communication channel are synchronized during logical or physical cancellation.
- **Configuration Logic:** Migrated internal flags to a `bitflags`-based `ConfigFlags` system to allow for easier future extensibility of `Bind` behavior.
- **Metadata:** Bumped dev-dependencies including `reqwest` and `walkers`.

### Fixed

- **State Machine Invariants:** Hardened `Bind::request` and `Bind::abort` to ensure the internal `State` and task handles move in lockstep, preventing "zombie" pending states where a task is running but the UI reports it as Idle.
- **Test Robustness:** Resolved race conditions in the test suite by implementing deterministic wait-barriers for task start and drop events.

## v0.2.6

- Bug fix for `request_every_sec` where `since_completed` was read before it as actually updated-- thanks @sectore !

## v0.2.5

- Update changelog

## v0.2.4

- Bump all dev dependencies
- Add many more tests for complete coverage
- Cleanup boilerplate in README.md example

## v0.2.3

### Added

- `Bind::ok_ref`, `Bind::err_ref`, and `Bind::take_ok` for direct access to success/error values without moving the entire `Result`.
- `Debug` implementation for `StateWithData<T, E>` to improve log/trace output.

### Fixed

- Do not panic when a pending task’s oneshot sender is dropped; treat as cancellation and return to `Idle`.
- `fill()` now also sets `last_start_time` to ensure coherent timing via `get_start_time()`, `get_complete_time()`, and friends.
- Ensure the oneshot receiver is dropped in `clear()` and when data is cleared during `poll()` (e.g., `retain=false` and not drawn last frame), preventing late results from reappearing.

### Examples

- Removed `unwrap()` calls; `eframe` example now returns errors instead of unwrapping.

### Tooling / Meta

- Clippy policy tightened for this release only: deny `todo!`, `panic!`, and `unwrap[_used]`.
- Bumped dev metadata/dependencies.

## v0.2.2

Adds convenience APIs, small perf/logging tweaks, better docs, and solid tests.

- **Release:** v0.2.2
- **Features (bind):** add `is_ok()`/`is_err()` state helpers
- **Features (bind):** add `request_every(Duration)` helper for periodic requests
- **Features (bind):** add getter + setter for `retain` policy
- **Performance (bind):** avoid double `poll()` in `request_every_sec()`
- **Observability (bind):** add trace log when spawning async request
- **Docs (bind):** document invariant for `state()` result mapping
- **Fix (lib):** stop running README examples as doctests
- **Tests:** add `bind_state_api` and `bind_transitions` suites
- **Meta:** bump dev deps
