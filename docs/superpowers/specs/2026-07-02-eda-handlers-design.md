# EDA Magic Handlers — Design Spec

**Goal:** Add 8 exploratory data analysis magic commands to orchard, wrapping
well-known R functions for summary, inspection, and comparison of data objects.

**Pattern:** Thin `MagicHandler` impls that call R functions via
`eval_r_captured()` or `eval_with_pkg_check()` and return `Output::Text`.

## Handler Mapping

| Handler | R Code | Pkg Check | Notes |
|---------|--------|-----------|-------|
| `%summary` | `summary(<args>)` | No | base R |
| `%glimpse` | `dplyr::glimpse(<args>)` | dplyr | Compact column view |
| `%describe` | `skimr::skim(<args>)` | skimr | Rich summary stats |
| `%missing` | `naniar::miss_summary(<args>)` | naniar | Missingness summary |
| `%corr` | `cor(<args>, use='pairwise.complete.obs')` | No | base R |
| `%freq` | `janitor::tabyl(<args>)` | janitor | Frequency tables |
| `%compare` | `waldo::compare(<args>, max_diffs=20)` | waldo | Object diff |
| `%sessioninfo` | `sessioninfo::session_info()` | sessioninfo | Reproducibility |

Handlers with optional package dependencies use the existing
`eval_with_pkg_check()` helper that checks `requireNamespace()` before calling
the function, returning a clear error if the package is not installed.

## Files

- **Create:** `src/magics/eda.rs` — all 8 handlers
- **Modify:** `src/magics/mod.rs` — add `pub mod eda;`
- **Modify:** `src/magic.rs` — add 8 registrations in `register_all()` (new "P9 — EDA" section)

## Testing

Each handler gets a parse+dispatch test that verifies the handler is registered
and returns the correct `Output` variant. These tests do not require R.
Full integration tests (requiring R + optional packages) are deferred.

## Registration

In `register_all()`, after the existing P8 section, add:

```rust
// P9 — EDA handlers
registry.register(Arc::new(crate::magics::eda::Summary));
registry.register(Arc::new(crate::magics::eda::Glimpse));
registry.register(Arc::new(crate::magics::eda::Describe));
registry.register(Arc::new(crate::magics::eda::Missing));
registry.register(Arc::new(crate::magics::eda::Corr));
registry.register(Arc::new(crate::magics::eda::Freq));
registry.register(Arc::new(crate::magics::eda::Compare));
registry.register(Arc::new(crate::magics::eda::SessionInfo));
```
