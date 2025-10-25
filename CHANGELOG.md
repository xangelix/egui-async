# Changelog

## v0.2.2

Adds convenience APIs, small perf/logging tweaks, better docs, and solid tests.

* **Release:** v0.2.2
* **Features (bind):** add `is_ok()`/`is_err()` state helpers
* **Features (bind):** add `request_every(Duration)` helper for periodic requests
* **Features (bind):** add getter + setter for `retain` policy
* **Performance (bind):** avoid double `poll()` in `request_every_sec()`
* **Observability (bind):** add trace log when spawning async request
* **Docs (bind):** document invariant for `state()` result mapping
* **Fix (lib):** stop running README examples as doctests
* **Tests:** add `bind_state_api` and `bind_transitions` suites
* **Meta:** bump dev deps

