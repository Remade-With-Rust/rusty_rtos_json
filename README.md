# rusty_rtos_json

[![Remade With Rust](https://img.shields.io/badge/Remade%20With-Rust-000?logo=rust&logoColor=fff)](https://github.com/remade-with-rust)
[![By Mata Network](https://img.shields.io/badge/by-Mata%20Network-5b2be0)](https://www.mata.network)
[![crates.io](https://img.shields.io/crates/v/rusty_rtos_json.svg)](https://crates.io/crates/rusty_rtos_json)
[![docs.rs](https://docs.rs/rusty_rtos_json/badge.svg)](https://docs.rs/rusty_rtos_json)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

A `no_std` JSON validator, the Kairos remake of coreJSON. MIT OR Apache-2.0.

**K7's second library**, and the first one in this family driven by bytes from
the network rather than by our own kernel.

- **Proven**: `JSON_Validate`, the strict ECMA-404 validator. It agrees with
  coreJSON v3.3.1 on **all 318 files of JSONTestSuite**, and passes the suite
  outright — 95/95 accepted, 188/188 rejected.
- **Zero allocation, on purpose.** The depth stack is a fixed 32-byte array;
  there is no heap on any path, so the crate builds and runs the same on a
  Cortex-M with no allocator at all.
- **A no-panic gate, because this one is fed by strangers.** Every other crate
  here is driven by our own kernel. A validator is driven by bytes off a
  socket, so "cannot panic on any input" is not tidiness, it is the security
  property.

**Known gaps.** `JSON_Search`, `JSON_SearchConst` and `JSON_Iterate` — the
query half of coreJSON — are **not written**. This crate validates; it does not
yet let you pull a value out by path. coreJSON has no serialiser and neither
does this.

- This package's plan: [docs/plans/rusty_rtos_json.md](https://github.com/Remade-With-Rust/rusty_rtos_json/blob/main/docs/plans/rusty_rtos_json.md)
- Every number: [docs/LEDGER.md](https://github.com/Remade-With-Rust/rusty_rtos_json/blob/main/docs/LEDGER.md)
- The family plan: Kairos [`docs/plans/rtos-mission.md`](https://github.com/Remade-With-Rust/kairos/blob/main/docs/plans/rtos-mission.md)

**Claims discipline:** this README makes no performance or capability claim that
is not backed by a test, a benchmark ledger entry, or a kill test recorded in
the plan. "Scaffold" means scaffold. "Sim only" means the sim port; "builds, not
flashed" means no chip has run it.

## Conformance

**318 of 318 files agree with `core_json.c`**, compiled verbatim from the
pinned checkout (v3.3.1 at `cffa492`), and the suite is passed outright.

```sh
cargo test -p rusty_rtos_json-core
```

The C arm's verdicts are checked in and the corpus is vendored, so this diffs
with no C toolchain and no network. Fetch the oracle itself with `kairos oracle
fetch --lib coreJSON`; the corpus is Nicolas Seriot's JSONTestSuite, pinned at
`1ef36fa` in [`ORACLES.md`](https://github.com/Remade-With-Rust/kairos/blob/main/ORACLES.md).

**Two tests, because they are two claims.** One compares our verdict against
**coreJSON's**, file by file; the other compares against **the suite's** — every
`y_` accepted, every `n_` rejected. They happen to be the same target here, and
that was *measured before either was adopted*: coreJSON itself scores 100 % on
the suite. Had it failed anywhere, "agree with the C" and "pass the suite"
would have pulled apart and one would have had to give. Keeping both means the
day that changes is visible rather than silently resolved.

The 35 `i_` files are where the standard leaves the answer to the
implementation, so the suite has no opinion and only the differential does.
coreJSON accepts 10 and rejects 25 — being *a* JSON parser does not determine
those, being *coreJSON* does.

**The corpus guard.** For a validator the rejections are the hard half:
accepting valid JSON is what any half-written scanner does, and every `n_` file
is a specific way to be wrong. A standing test fails if the corpus stops being
mostly rejections. That is the fourth shape of that guard here, after
`heap_4`'s (too few refusals), `heap_1`'s (never exhausted) and `backoff`'s (an
unvisited branch), and all four say the same thing: a differential whose
workload cannot fail is a differential about nothing.

**Poison-proven on four behaviours**, each caught by a different set of files:

* **over-long UTF-8** — accepting a non-shortest encoding fails 2 files;
* **lone surrogates** — accepting an unpaired high surrogate escape fails 3;
* **trailing commas** — allowing one fails 2;
* **leading zeros** — allowing `01` fails 3.

## The no-panic gate

The crate forbids `unsafe` and denies `unwrap`, `expect` and `panic`, so a
panic could only come from arithmetic that overflows, an index out of range, or
a slice shorter than something assumed. The lints catch the *shapes*; these go
after the reachability:

| test | what it feeds in |
|---|---|
| arbitrary bytes | every length 0..256 of unstructured noise |
| JSON-shaped noise | 20,000 documents drawn from JSON's own alphabet, which reaches far deeper than uniform noise |
| every truncation | six valid documents cut at every offset, 202 slices — the shape a packet boundary actually makes |
| every single-byte corruption | one valid document, all 46 positions × 35 interesting bytes |
| nesting past the limit | 32, 33, 64, 1,000 and 10,000 brackets, both kinds and mixed |
| the stress files | including the corpus's 100,000 opening brackets |

**22,085 documents in all**, and every one of them deterministic: the pseudo-
random arms use a written-out LCG rather than a system source, so a failure is
reproducible from the seed alone on any machine.

**Broken on purpose before it was believed.** A no-panic gate that has never
failed is indistinguishable from one that cannot fail, so three panics were
introduced deliberately. Two were caught at once — an unchecked slice in the
literal scanner (found by the truncation of `true`) and an unchecked index into
the depth stack (found by the nesting test and the stress file).

The third was not caught, and that is the interesting result. Making the
universal byte reader panic on any read past the end leaves **all eleven tests
passing** — so no scanner reads out of bounds in the first place, and the
`Option` it returns is defence in depth rather than the thing keeping this
safe. The two bounds that *are* load-bearing are the two above, and both are
proven to be. That measurement is recorded next to the function, because it
holds only while every caller is right and a later edit would lose it silently.

## Using it

```rust
use rusty_rtos_json::{is_valid, validate, Validity};

assert!(is_valid(br#"{"a":[1,2,{"b":"x"}],"c":true}"#));

// Refusals are distinguished, not collapsed to a bool: a caller fixes a
// truncated document and an illegal one in different ways.
assert_eq!(validate(b"{"),   Validity::Partial);
assert_eq!(validate(b"{]"),  Validity::Illegal);
assert_eq!(validate(b""),    Validity::BadParameter);
```

A bare scalar at the top level is a valid document — that is ECMA-404 rather
than RFC 4627, it is coreJSON's default, and it is the single most likely place
for a reader to think the parser is too lax.

Nesting deeper than `MAX_DEPTH` (32, the C's) answers
`Validity::MaxDepthExceeded`. The bound exists because the stack is explicit:
recursion here would meet a 100,000-bracket document with a stack overflow,
which is not a panic and cannot be caught.

## Performance

No rows. Nothing here is measured yet, and the ledger has no entry for this
crate — what matters about a validator is first that it agrees with the C,
which the 318 files above establish.

## Portability

Builds `no_std` with no default features on `thumbv7em-none-eabihf` and
`riscv32imac-unknown-none-elf` (both verified), and CI holds it to
`thumbv8m.main-none-eabihf` and `riscv32imafc-unknown-none-elf` as well. A
build claim, not a behaviour claim: no chip has run this yet.

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
the C kernel's own trace. `rusty_rtos_json` is one of the K7 libraries: the validator is done and proven against the C, the query half is not written.

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
