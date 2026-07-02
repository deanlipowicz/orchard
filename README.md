# orchard — Modern R REPL with IPython-style Magic Commands

**What this is:** An enhanced terminal REPL for the R statistical programming
language, written in Rust. Replaces the upstream Python radian REPL with a
faster, self-contained binary. Currently 49 magic handlers and 238 tests.

**Related documents:**
- Development roadmap: `docs/development-plan.md`
- Chronological log: `docs/developer-log.md`
- Feature comparison: `docs/review-2026-07-01.md`
- Feature specs: `docs/superpowers/specs/`
- Implementation plans: `docs/superpowers/plans/`

### Status

**Status:** v0.9 | 49 magic handlers | ~238 tests pass | Linux only

### Quick Start

```bash
cargo build --release
./target/release/orchard -q
```

### Documentation

- `docs/` — port plan, design history, developer log, review notes
- `docs/superpowers/` — development plans and specs

### License

MIT OR Apache-2.0
