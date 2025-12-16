# Changelog

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
