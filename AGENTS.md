# Agent instructions

## Reading order

1. `docs/PROJECT_CONTEXT.md`
2. `docs/DECISIONS.md`
3. `TODO.md`
4. `README.md` and the crate you are changing

Executed 2026-08-21 design/plan (durable rules live in DECISIONS):
`docs/specs/2026-08-21-rngkit-core-design.md`,
`docs/plans/2026-08-21-rngkit-core-plan.md`.

## Verified commands (Windows host, through 2026-08-22)

From the workspace root. Stable is rustc 1.97.1; MSRV toolchain `1.85.0` is
installed. Do not install a missing toolchain silently.

```text
cargo fmt --all -- --check
cargo metadata --no-deps
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --doc --all-features
cargo +1.85.0 check --workspace --all-targets --all-features
cargo +1.85.0 test --workspace --all-targets --all-features
cargo tree --workspace --all-features
cargo check -p rngkit-sources --no-default-features
cargo check -p rngkit-sources --no-default-features --features bitb
cargo check -p rngkit-sources --no-default-features --features trng3
cargo check -p rngkit-sources --no-default-features --features rdseed
cargo check -p rngkit-sources --no-default-features --features pseudo
git diff --check
```

Physical tests (Windows host, through 2026-08-22):

```text
cargo test -p rngkit-sources --test hardware -- --ignored --test-threads=1 --nocapture
```

Do not infer Linux or other-device support from those results.

## Conventions

- Six packages under `crates/`. No Tauri, Tokio, GUI, or other async runtime.
- Default tests must not enumerate or open hardware.
- Do not modify the four external source crates.
- Do not persist serials, OS paths, seeds, or PRNG state.
- Cumulative Z is descriptive; `±1.96` lines are visual references only.
- Do not commit, push, or create a remote unless the user authorizes it.

## Context maintenance

Update `TODO.md` after relevant work. Update `docs/DECISIONS.md` when a durable
decision changes. Keep memory files dense.
