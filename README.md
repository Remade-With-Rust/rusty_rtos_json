# rusty_rtos_json

[![Remade With Rust](https://img.shields.io/badge/Remade%20With-Rust-000?logo=rust&logoColor=fff)](https://github.com/remade-with-rust)
[![By Mata Network](https://img.shields.io/badge/by-Mata%20Network-5b2be0)](https://www.mata.network)
[![crates.io](https://img.shields.io/crates/v/rusty_rtos_json.svg)](https://crates.io/crates/rusty_rtos_json)
[![docs.rs](https://docs.rs/rusty_rtos_json/badge.svg)](https://docs.rs/rusty_rtos_json)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

A `no_std` JSON parser and serialiser, the Kairos remake of coreJSON. MIT OR Apache-2.0.

**This crate is a scaffold.** The layout, feature ladder, lint policy and CI
gates exist; there is no implementation behind them yet. It is listed here so
the family's shape is visible, and the README says so rather than implying
otherwise.

- **What exists**: the crate layout, the `no_std` / `alloc` / `std` feature
  ladder, the workspace lint policy, `cargo deny`, and the CI gate every Kairos
  package shares.
- **What does not**: the implementation, and its oracle — JSONTestSuite and coreJSON's own vectors, plus a no-panic fuzz gate. This is
  milestone **K7**, which follows the kernel, the ports and the C ABI.

**Known gaps.** Everything above the scaffold. Do not depend on this crate.

- This package's plan: [docs/plans/rusty_rtos_json.md](https://github.com/Remade-With-Rust/rusty_rtos_json/blob/main/docs/plans/rusty_rtos_json.md)
- Every number: [docs/LEDGER.md](https://github.com/Remade-With-Rust/rusty_rtos_json/blob/main/docs/LEDGER.md)
- The family plan: Kairos [`docs/plans/rtos-mission.md`](https://github.com/Remade-With-Rust/kairos/blob/main/docs/plans/rtos-mission.md)

**Claims discipline:** this README makes no performance or capability claim that
is not backed by a test, a benchmark ledger entry, or a kill test recorded in
the plan. "Scaffold" means scaffold. "Sim only" means the sim port; "builds, not
flashed" means no chip has run it.

## Conformance

**None yet, and that is the honest answer.** The Kairos rule is that a README
makes no capability claim that is not backed by a test, a benchmark ledger entry
or a kill test recorded in the plan. This section stays empty until K7's oracle
passes.

## Using it

Not yet. Track K7 in the
[mission plan](https://github.com/Remade-With-Rust/kairos/blob/main/docs/plans/rtos-mission.md).

## Performance

No rows. Nothing here is measured.

## Portability

Builds `no_std` on host, `thumbv7m-none-eabi`,
`riscv32imac-unknown-none-elf` and `xtensa-esp32s3-none-elf`. A build claim, not
a behaviour claim.

## Layout

```text
crates/rusty_rtos_json          facade: re-exports + prelude; the crate you depend on
crates/rusty_rtos_json-core     no_std (+ alloc); forbid(unsafe); types, traits, algorithms
firmware/                per-chip example projects, excluded from the workspace
docs/plans/              this package's plan and its hardening audit
docs/LEDGER.md           every number, with its method line
```

## Build

```sh
cargo test --workspace                                   # host: the tests
cargo check -p rusty_rtos_json-core --no-default-features \
  --target thumbv7em-none-eabihf                         # Cortex-M4F class, no alloc
cargo check -p rusty_rtos_json-core --no-default-features --features alloc \
  --target riscv32imac-unknown-none-elf                  # ESP32-C6 class, with alloc
```

CI holds the core to `thumbv7em-none-eabihf`, `thumbv8m.main-none-eabihf`,
`riscv32imac-unknown-none-elf` and `riscv32imafc-unknown-none-elf`, with and
without `alloc`, plus `cargo deny check`. Firmware examples (Xtensa needs the
esp toolchain; Cortex-M and RISC-V work on stable) are built from their own
directories under `firmware/`.

## Part of Remade With Rust

This crate is part of **[Kairos](https://github.com/Remade-With-Rust/kairos)** —
FreeRTOS remade in memory-safe Rust, as independent packages that expose the API
a FreeRTOS developer already knows and prove every scheduling decision against
the C kernel's own trace. `rusty_rtos_json` is one of the K7 libraries, and is not started.

The family:
[`rusty_rtos_core`](https://crates.io/crates/rusty_rtos_core),
[`rusty_rtos_kernel`](https://crates.io/crates/rusty_rtos_kernel),
[`rusty_rtos_port`](https://crates.io/crates/rusty_rtos_port),
[`rusty_rtos_heap`](https://crates.io/crates/rusty_rtos_heap),
`rusty_rtos-capi` and `rusty_rtos_demo` (neither published yet). Also
check out the rest of
**[github.com/remade-with-rust](https://github.com/remade-with-rust)**.

## About Mata Network

<!-- ORG BOILERPLATE — keep identical across repos -->

[Mata Network](https://www.mata.network) builds sovereign, self-hostable
infrastructure. **Remade With Rust** is our open-source home for the
permissively-licensed building blocks that work depends on.

<!-- /ORG BOILERPLATE -->

## License

MIT OR Apache-2.0, at your option. FreeRTOS is MIT-licensed by Amazon.com,
Inc. or its affiliates; this crate remakes its API and behaviour from the
published sources and links no FreeRTOS code.

---

<!-- HARDENING-TABLE:BEGIN generated by use-protection-please — edit docs/plans/use-protection-please.md, not this block -->
## Hardening status

**Tier** critical-path · **Audited** 2026-09-16 (v0.1.0 release pass) · **v1.0.0 gates** 7/17 · [Full checklist](https://github.com/Remade-With-Rust/rusty_rtos_json/blob/main/docs/plans/use-protection-please.md)

`██████░░░░░░░░░░░░░░` **31%** &nbsp;·&nbsp; 11 Completed · 0 Scheduled · 25 Incomplete · 19 N/A

| Phase | ✅ Completed | 🗓 Scheduled | ⬜ Incomplete | · N/A |
|---|--:|--:|--:|--:|
| 0 — Threat modeling | 0 | 0 | 2 | 0 |
| 1 — Toolchain | 2 | 0 | 2 | 0 |
| 2 — Supply chain | 5 | 0 | 3 | 0 |
| 3 — Code level | 3 | 0 | 4 | 0 |
| 4 — Static analysis | 0 | 0 | 1 | 0 |
| 5 — Dynamic analysis | 0 | 0 | 3 | 0 |
| 6 — Fuzzing and properties | 0 | 0 | 4 | 0 |
| 7 — Formal verification | 0 | 0 | 1 | 0 |
| 8 — Build and binary | 0 | 0 | 1 | 1 |
| 9 — Runtime privilege | 0 | 0 | 0 | 1 |
| 10 — Cryptography | 0 | 0 | 0 | 3 |
| 11 — CI/CD, release, and operations | 1 | 0 | 4 | 0 |
| 12 — Compliance controls | 0 | 0 | 0 | 14 |
| **Total** | **11** | **0** | **25** | **19** |

Gates waived for 0.x are listed with their reasons in the plan's "v0.1.0 release decision" section — an Incomplete gate not listed there is an omission, not a decision.

**Architect** — [Tim Almond](https://github.com/Ttimmahlax) — accountable for this unit's security design; rendered
<!-- HARDENING-TABLE:END -->
