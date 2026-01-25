# Changelog

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
