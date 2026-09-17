# rusty_rtos_json — package plan

**One sentence:** coreJSON remade in Rust: the strict ECMA-404 validator
(done, and agreeing with the C on all 318 JSONTestSuite files) and the
query-by-path search (not written), zero allocation, no_std,
forbid(unsafe), with the no-panic gate every parser in the family carries.

Family plan: Kairos `docs/plans/rtos-mission.md` (umbrella repo) — its §2.1
names what this package remakes, wraps and never touches; its §6 carries the
phase this package's kill test belongs to. This file obeys that one.

Written 2026-09-09. Status: **the validator is built and proven** — 318/318
files agree with `core_json.c` and JSONTestSuite passes outright; the query
half (`JSON_Search`, `JSON_Iterate`) is not written. Last touched 2026-09-16.

---

## 1. What it is, what it is not

**Is:** the Rust remake of the FreeRTOS component named above, exposing the
names a FreeRTOS developer already knows, with the C original as the oracle.

**Is not:** a binding to the C code, a fork of it, or a place where a chip's
registers are touched (that is a port crate).

## 2. The laws this package encodes

1. The core is `no_std` (+ `alloc`), `forbid(unsafe)`, arch-agnostic.
2. Every parser that takes bytes from a wire, a store or a bus has a
   `tests/no_panic.rs` from the day it exists.
3. Every claim has a kill test or a ledger row; the README copies this plan
   and never upgrades it.
4. Feature ladder `std` ⊃ `alloc` ⊃ core-only; CI proves the two bare-metal
   rungs on four targets on every push.

## 3. The surface as built

`JSON_Validate`, and nothing else yet.

| ours | coreJSON | note |
|---|---|---|
| `validate(&[u8]) -> Validity` | `JSON_Validate` | the four outcomes, kept distinct |
| `is_valid(&[u8]) -> bool` | — | the common case, for readability at the call site |
| `Validity::{Valid, Illegal, MaxDepthExceeded, Partial, BadParameter}` | `JSONStatus_t` | a caller fixes a truncated document and an illegal one in different ways |
| `MAX_DEPTH` (32) | `JSON_MAX_DEPTH` | the explicit stack's bound, the C's value |

Zero allocation: the depth stack is `[u8; 32]` on the frame and there is no
heap on any path. The core builds with no default features on
`thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf`.

**Not built:** `JSON_Search`, `JSON_SearchConst`, `JSON_SearchT` and
`JSON_Iterate`. The README says so; the crate description on crates.io says so.

## 4. Roadmap

| Milestone | Adds | Driven by | Kill test |
|---|---|---|---|
| scaffold | the shape | K0 | a clean clone builds alone; CI green ✅ |
| **validator** | `JSON_Validate` | K7 | **318/318 files agree with `core_json.c`, and JSONTestSuite passes outright (95/95, 188/188)** ✅ |
| search | `JSON_Search`, `JSON_SearchConst`, `JSON_Iterate` | K7 | a query differential against the C over the same corpus, plus the no-panic gate extended to the query path |

## 5. Deliberately absent

| Absent | Why |
|---|---|
| a serialiser | coreJSON has none. A crate that validates on the way in and writes on the way out is two libraries; the writer belongs with `rusty_json_turbo` under `alloc`. |
| a DOM | the whole point of coreJSON is querying a byte buffer in place. Materialising a tree needs an allocator and would make the `no_std` claim conditional. |
| recursion | a 100,000-bracket document meets an explicit, bounded stack and gets `MaxDepthExceeded`. Recursion would meet it with a stack overflow, which is not a panic and cannot be caught — the no-panic gate could not see it. |
| any `unsafe` | `forbid(unsafe_code)`, and the validator never needed it. |
| our own UTF-8 decoder on the `std` path | the scanners work on bytes throughout, as the C does; there is nothing to convert. |

## 6. Risks

| Risk | Mitigation |
|---|---|
| The corpus is the denominator, and an unpinned corpus makes a percentage meaningless. | `test_parsing/` is vendored into `oracle/` with its MIT licence, and pinned at `1ef36fa` in the umbrella's `ORACLES.md`. The file counts are asserted (95 / 188 / 35), so a corpus that changed under us fails rather than re-scoring. |
| "Agrees with the C" and "passes the suite" could diverge, and one would silently win. | They are two separate tests. coreJSON scoring 100 % was **measured before either was adopted**, not assumed. |
| A no-panic gate that has never failed is indistinguishable from one that cannot fail. | Three panics were introduced deliberately; two were caught. The third was not, and that is recorded as a property rather than filed as a passing test — see the decision log. |
| The 35 `i_` files are implementation-defined, so nothing but the differential pins them. | That is exactly what the differential is for; being *a* JSON parser does not determine them, being *coreJSON* does. |
| `MAX_DEPTH` is a compile-time constant, and a caller who needs more has no recourse. | The C has the same constant. If it needs to move it becomes a `parameterizing-a-constant` job with the inversion check, not a quiet edit. |

## 7. Decision log

| Date | Decision |
|---|---|
| 2026-09-09 | Stamped from the Kairos template; obeys the family plan. |
| 2026-09-09 | **Relation to the house `rusty_json_turbo`** (serde_json forked and made fast; lib name `serde_json`; `no_std + alloc`, checked on the Kairos bare-metal targets in the umbrella's `tools/house-gate`): this package is the coreJSON *API* — the zero-allocation validator and `JSON_Search` over a byte buffer, `forbid(unsafe)`, no `alloc` — which serde_json's model (an `alloc`-backed `Value` and boxed errors) cannot provide; typed (de)serialization under `alloc` goes through `rusty_json_turbo` behind a `serde` feature, never a second parser for that job. The umbrella's `docs/HOUSE-STACK.md` carries the pin and the gate. |
| 2026-09-16 | **Measured coreJSON against JSONTestSuite before adopting either.** 95/95 `y_` accepted, 188/188 `n_` rejected, 10/35 `i_` accepted. Only because that came back 100 % are "match the C" and "pass the suite" the same target; both tests are kept so the day it stops being true is visible. |
| 2026-09-16 | **The corpus is vendored, not fetched** (746 KB, MIT, licence alongside). The differential has to run in CI on a box with no network and no C toolchain, which is the same arrangement as the kernel traces and `backoff.trace`. |
| 2026-09-16 | **The validator was poison-proved on four behaviours**, each caught by a different file set: over-long UTF-8 (2 files), lone surrogates (3), trailing commas (2), leading zeros (3). These are the four places a JSON reimplementation drifts, and each now fails loudly if it drifts. |
| 2026-09-16 | **The no-panic gate was poison-proved three ways, and one of the three did not fail — deliberately recorded as a finding rather than quietly dropped.** An unchecked slice in the literal scanner and an unchecked index into the depth stack were both caught immediately. But replacing the universal byte reader `at()` with a panicking index left **all eleven tests passing**, which means no scanner reads past the end in the first place: the `Option` is defence in depth, not the thing keeping this safe. It stays, because the property holds only while every caller is right and a later edit would lose it silently. The measurement is written next to the function so it can be re-run. |
| 2026-09-16 | **The crates.io description was corrected, not left aspirational.** It claimed "the query-by-path search", which is not written. A published description is a capability claim and falls under the same discipline as the README. |
