# rusty_rtos_json

[![Remade With Rust](https://img.shields.io/badge/Remade%20With-Rust-000?logo=rust&logoColor=fff)](https://github.com/remade-with-rust)
[![By Mata Network](https://img.shields.io/badge/by-Mata%20Network-5b2be0)](https://www.mata.network)
[![crates.io](https://img.shields.io/crates/v/rusty_rtos_json.svg)](https://crates.io/crates/rusty_rtos_json)
[![docs.rs](https://docs.rs/rusty_rtos_json/badge.svg)](https://docs.rs/rusty_rtos_json)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

A `no_std` JSON validator and in-place query engine, the Kairos remake of
coreJSON. It agrees with the C on all 318 files of JSONTestSuite, and passes
the suite outright.

- **Proven, the validator**: `JSON_Validate`, strict ECMA-404. It agrees with
  coreJSON v3.3.1 on **all 318 files of JSONTestSuite**, and passes the suite
  outright — 95/95 accepted, 188/188 rejected.
- **Proven, the query engine**: `JSON_SearchConst` and `JSON_Iterate`, over
  **2,124 queries and 348 iterations** compared against the C as a 3,089-line
  trace — status, offset, length and type, every time.
- **Zero allocation, on purpose.** The depth stack is a fixed 32-byte array;
  there is no heap on any path, so the crate builds and runs the same on a
  Cortex-M with no allocator at all.
- **A no-panic gate, because this one is fed by strangers.** Every other crate
  here is driven by our own kernel. A validator is driven by bytes off a
  socket, so "cannot panic on any input" is not tidiness, it is the security
  property.

**Known gaps.** coreJSON has no serialiser and neither does this. `JSON_SearchT`
is absent on purpose: it is a cast of `JSON_SearchConst` that exists only to
let C callers pass a mutable buffer, and Rust needs no such twin.

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

## Querying

**2,124 queries and 348 iterations agree with `core_json.c`**, compared as a
3,089-line trace, line for line.

```sh
cargo test -p rusty_rtos_json-core --test query
```

`search` takes a dotted, bracketed path and hands back a **sub-slice of the
buffer you already have** — no tree, no copy, no allocation. `a.b[2].c` is the
key `c` of the third element of the array at `a.b`.

```rust
use rusty_rtos_json::{search, pairs, Kind};

let doc = br#"{"a":{"b":[10,20,{"c":"hi"}]}}"#;
assert_eq!(search(doc, b"a.b[1]").unwrap().value, b"20");

let found = search(doc, b"a.b[2].c").unwrap();
assert_eq!(found.kind, Kind::String);   // quotes stripped, as the C does
assert_eq!(found.value, b"hi");

// Walk a collection instead. An array yields values with no keys.
for pair in pairs(br#"{"x":1,"y":2}"#) {
    let _ = (pair.key, pair.value, pair.kind);
}
```

**Two refusal types, not one.** The C has a single `JSONStatus_t` and each
function documents the subset it can return. Here `search` can only answer
`Missing` or `BadQuery`, and `iterate` adds `NotACollection` — so they have
different types and the impossible variant is not there to be matched on. The
differential maps both back to the C's names, which is what makes the arms
comparable at all.

**Neither entry point validates first**, and neither does the C. A query walks
whatever bytes it is handed. That is why the differential runs both of them
over all 318 corpus files including the 188 that are malformed on purpose:
walking a broken document is exactly where a reimplementation reads off the
end. 70 of those malformed files still answer a query successfully, and every
one of those answers had to match.

**Poison-proven on five behaviours**, each a real place a reimplementation
drifts:

* a string's **quotes are stripped** from the value, and the length shortened
  by two;
* a **trailing separator** (`a.`) is a `BadQuery`, which is one `- 1` in the C;
* the **first** duplicate key wins, not the last;
* a **huge array index latches to -1** rather than wrapping, so
  `[99999999999999999999]` is refused instead of reading element zero;
* an **array element reports no key**, which the C signals with a NULL pointer.

A sixth poison did **not** fire, and that is recorded rather than dropped:
swapping the order in which a value is tried as a scalar and as a collection
changes nothing anywhere in the 3,089 lines. The two scanners are disjoint on
their first byte and neither moves the cursor when it fails, so the order is
free — and a unit test now pins both halves of that, because an accidental
property nobody checks is one edit away from being false.

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
| **the query surface** | 20,000 random documents x random queries; 20,000 iterations driven to exhaustion; every truncation and corruption of a nested document through 8 queries; all 318 corpus files through both entry points |

**73,005 documents in all** — 22,085 through the validator and 50,920
through the query engine. Every one of them is deterministic: the
pseudo-random arms use a written-out LCG rather than a system source, so a
failure is reproducible from the seed alone on any machine.

**Broken on purpose before it was believed.** A no-panic gate that has never
failed is indistinguishable from one that cannot fail, so three panics were
introduced into the validator deliberately. Two were caught at once — an unchecked slice in the
literal scanner (found by the truncation of `true`) and an unchecked index into
the depth stack (found by the nesting test and the stress file).

The third was not caught, and that is the interesting result. Making the
universal byte reader panic on any read past the end leaves **all eleven tests
passing** — so no scanner reads out of bounds in the first place, and the
`Option` it returns is defence in depth rather than the thing keeping this
safe. The two bounds that *are* load-bearing are the two above, and both are
proven to be. That measurement is recorded next to the function, because it
holds only while every caller is right and a later edit would lose it silently.

**The query half needed a different kind of test, and it found a different kind
of bug.** `iterate` carries a cursor the *caller* owns, so a version that failed
to advance it would not panic and would not answer wrongly — it would hang, and
a hang is the one failure a test runner reports as "still running" rather than
as a bug. A standing test asserts the cursor strictly advances on every success,
over 20,000 random documents. Stopping the cursor on purpose turns an infinite
loop into a named assertion with the offending document printed.

Two more poisons there did **not** fire, and they say the same thing the reader
poison did: taking `multiSearch`'s narrowed sub-slice unchecked, and taking the
returned slice unchecked, both leave every test passing. The narrowing
arithmetic only ever shrinks, so those bounds cannot fail — they stay as
`get` rather than indexing because that is what keeps "cannot panic" a property
of the code rather than of the current call graph.

## Using it

The validator; [Querying](#querying) has the other half.

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

No speed row and no size row: nothing here has been benchmarked, and nothing
has run on a chip. The ledger does carry this crate's **counts** — the 318
files, the 2,124 queries, the 73,005 documents — because a count is a number
and belongs there with its method, the same as a timing would.

## Portability

Builds `no_std` with no default features on `thumbv7em-none-eabihf` and
`riscv32imac-unknown-none-elf` (both verified), and CI holds it to
`thumbv8m.main-none-eabihf` and `riscv32imafc-unknown-none-elf` as well. A
build claim, not a behaviour claim: no chip has run this yet.

## Layout

```text
crates/rusty_rtos_json          facade: re-exports + prelude; the crate you depend on
crates/rusty_rtos_json-core     no_std (+ alloc); forbid(unsafe); types,
traits, algorithms
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
the C kernel's own trace. `rusty_rtos_json` is one of the K7 libraries, and **both halves are done**:
the validator passes all 318 files of JSONTestSuite, and the query engine
agrees with the C over 2,124 queries and 348 iterations.

**Where this sits for Mata.** Kairos is the real-time layer on the device
itself, and [`rusty_rtos_mqtt`](https://github.com/Remade-With-Rust/rusty_rtos_mqtt) is the way out of it.
Paired with the **MATA distributed cloud**, robotics and sensor data has two
routes — read it on the machine, or reach it through the cloud — with the same
memory-safe crates at both ends.

The family:
[`rusty_rtos_core`](https://crates.io/crates/rusty_rtos_core) (the shared vocabulary),
[`rusty_rtos_kernel`](https://crates.io/crates/rusty_rtos_kernel) (the scheduler),
[`rusty_rtos_port`](https://crates.io/crates/rusty_rtos_port) (the architecture seam),
[`rusty_rtos_heap`](https://crates.io/crates/rusty_rtos_heap) (the allocators),
[`rusty_rtos_json`](https://github.com/Remade-With-Rust/rusty_rtos_json) (coreJSON),
[`rusty_rtos_sntp`](https://github.com/Remade-With-Rust/rusty_rtos_sntp) (coreSNTP),
[`rusty_rtos_mqtt`](https://github.com/Remade-With-Rust/rusty_rtos_mqtt) (coreMQTT),
[`rusty_rtos_backoff`](https://github.com/Remade-With-Rust/rusty_rtos_backoff) (backoffAlgorithm),
[`rusty_rtos-capi`](https://github.com/Remade-With-Rust/rusty_rtos-capi) (the C ABI) and
[`rusty_rtos_demo`](https://github.com/Remade-With-Rust/rusty_rtos_demo) (the conformance corpus).
The last six are on GitHub and not yet on crates.io. Also check out
the rest of **[github.com/remade-with-rust](https://github.com/remade-with-rust)**.

## About Mata Network

<!-- ORG BOILERPLATE — keep identical across repos -->

**[Mata Network](https://www.mata.network/)** builds sovereign, self-hostable
privacy infrastructure — *"stop sacrificing your privacy for convenience"*:
wallet & identity, a password manager, a contact manager, and a browser
extension that stops your information leaking as you browse.

**Remade With Rust** is our open-source home for the permissively-licensed
building blocks that work depends on — including
[remade_ffmpeg_rs](https://github.com/Remade-With-Rust/remade_ffmpeg_rs) (the
FFmpeg alternative) and [FFAI](https://github.com/Remade-With-Rust/FFAI) (the
AI media toolkit).

→ **[www.mata.network](https://www.mata.network/)**

<!-- /ORG BOILERPLATE -->

## License

MIT OR Apache-2.0, at your option. FreeRTOS is MIT-licensed by Amazon.com,
Inc. or its affiliates; this crate remakes its API and behaviour from the
published sources and links no FreeRTOS code.

---

<!-- HARDENING-TABLE:BEGIN generated by use-protection-please — edit
docs/plans/use-protection-please.md, not this block -->
## Hardening status

**Tier** critical-path · **Audited** 2026-09-16 (v0.1.0 release pass) · **v1.0.0 gates** 7/17 · [Full checklist](https://github.com/Remade-With-Rust/rusty_rtos_json/blob/main/docs/plans/use-protection-please.md)

`██████░░░░░░░░░░░░░░` **31%** &nbsp;·&nbsp; 11 Completed · 0 Scheduled · 25
Incomplete · 19 N/A

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

Gates waived for 0.x are listed with their reasons in the plan's "v0.1.0
release decision" section — an Incomplete gate not listed there is an
omission, not a decision.

**Architect** — [Tim Almond](https://github.com/Ttimmahlax) — accountable for this unit's security design; rendered
<!-- HARDENING-TABLE:END -->
