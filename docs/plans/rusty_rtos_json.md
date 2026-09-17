# rusty_rtos_json — package plan

**One sentence:** coreJSON remade in Rust — the strict ECMA-404 validator and
the query-by-path search, both diffed against the C, zero allocation, no_std,
forbid(unsafe), with the no-panic gate every parser in the family carries.

Family plan: Kairos `docs/plans/rtos-mission.md` (umbrella repo) — its §2.1
names what this package remakes, wraps and never touches; its §6 carries the
phase this package's kill test belongs to. This file obeys that one.

Written 2026-09-09. Status: **both halves are built and proven** — the
validator agrees with `core_json.c` on 318/318 files and passes JSONTestSuite
outright; the query engine agrees over 2,124 queries and 348 iterations.
Last touched 2026-09-16.

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

### The query half

| ours | coreJSON | note |
|---|---|---|
| `search(&[u8], &[u8]) -> Result<Match, NotFound>` | `JSON_SearchConst` | a dotted, bracketed path |
| `Match { offset, value, kind }` | the three out-parameters | `value` borrows the input, so the lifetime the C leaves to the reader is checked |
| `iterate(&[u8], &mut usize, &mut usize) -> Result<Pair, Stopped>` | `JSON_Iterate` | the C's shape, cursors and all |
| `pairs(&[u8]) -> Pairs` | — | the same thing as a Rust `Iterator`, which is what most callers want |
| `Pair { key, key_offset, offset, value, kind }` | `JSONPair_t` | `key` is `None` for an array element, which the C signals with NULL |
| `Kind` | `JSONTypes_t` | without `JSONInvalid`: nothing here can return it, so it is not a variant |
| `NotFound`, `Stopped` | `JSONStatus_t` | two enums, because `search` cannot answer `IllegalDocument` and should not be matched as though it could |

**Not built, deliberately:** `JSON_SearchT`, which is a cast of
`JSON_SearchConst` so a C caller can pass a mutable buffer. Rust needs no twin
for that. And no serialiser, because coreJSON has none.

## 4. Roadmap

| Milestone | Adds | Driven by | Kill test |
|---|---|---|---|
| scaffold | the shape | K0 | a clean clone builds alone; CI green ✅ |
| **validator** | `JSON_Validate` | K7 | **318/318 files agree with `core_json.c`, and JSONTestSuite passes outright (95/95, 188/188)** ✅ |
| **search** | `JSON_SearchConst`, `JSON_Iterate` | K7 | **2,124 queries and 348 iterations agree with the C, compared as a 3,089-line trace; the no-panic gate extended to the query path, including a liveness assert** ✅ |

## 5. Deliberately absent

| Absent | Why |
|---|---|
| a serialiser | coreJSON has none. A crate that validates on the way in and writes on the way out is two libraries; the writer belongs with `rusty_json_turbo` under `alloc`. |
| a DOM | the whole point of coreJSON is querying a byte buffer in place. Materialising a tree needs an allocator and would make the `no_std` claim conditional. |
| recursion | a 100,000-bracket document meets an explicit, bounded stack and gets `MaxDepthExceeded`. Recursion would meet it with a stack overflow, which is not a panic and cannot be caught — the no-panic gate could not see it. |
| any `unsafe` | `forbid(unsafe_code)`, and neither half needed it. |
| `JSON_SearchT` | a cast of `JSON_SearchConst` that lets a C caller pass a `char *` instead of a `const char *`. Rust has no such distinction to paper over. |
| an escape in the query grammar | a key containing `.` or `[` cannot be reached by a query. That is the C's limitation and it is kept, because inventing an escape would make our queries and the C's mean different things. Two documents in the differential exist to pin it. |
| our own UTF-8 decoder on the `std` path | the scanners work on bytes throughout, as the C does; there is nothing to convert. |

## 6. Risks

| Risk | Mitigation |
|---|---|
| The corpus is the denominator, and an unpinned corpus makes a percentage meaningless. | `test_parsing/` is vendored into `oracle/` with its MIT licence, and pinned at `1ef36fa` in the umbrella's `ORACLES.md`. The file counts are asserted (95 / 188 / 35), so a corpus that changed under us fails rather than re-scoring. |
| "Agrees with the C" and "passes the suite" could diverge, and one would silently win. | They are two separate tests. coreJSON scoring 100 % was **measured before either was adopted**, not assumed. |
| A no-panic gate that has never failed is indistinguishable from one that cannot fail. | Three panics were introduced deliberately; two were caught. The third was not, and that is recorded as a property rather than filed as a passing test — see the decision log. |
| The 35 `i_` files are implementation-defined, so nothing but the differential pins them. | That is exactly what the differential is for; being *a* JSON parser does not determine them, being *coreJSON* does. |
| A query engine can HANG rather than fail: `iterate` carries a caller-owned cursor, and one that stops advancing loops for ever. A hang is reported as "still running", not as a bug. | A standing test asserts the cursor strictly advances on every success, over 20,000 random documents. Stalling it on purpose turns the infinite loop into a named assertion with the offending document printed. |
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
| 2026-09-16 | **The query half gets two refusal enums, not the C's one.** `search` can only answer `Missing` or `BadQuery`; `iterate` adds `NotACollection`. The C documents which subset each function returns and trusts the reader; a type that cannot express the impossible variant does not need to be trusted. The differential maps both back to the C's names, which is what keeps the arms comparable. |
| 2026-09-16 | **`Pair` carries `key_offset` although the C does not.** The C hands back a pointer and leaves the caller to subtract. The first version of the differential did exactly that — pointer arithmetic in a test, in a crate that forbids `unsafe` — and needing it was the sign the field was missing. |
| 2026-09-16 | **The C driver's 32-pair cap was dead code in both arms, and that was a defect in the WORKLOAD.** No document produced more than five pairs, so a cap that exists to stop one corpus file dominating the trace had never fired. A cap that never fires cannot be trusted to fire symmetrically, and it is exactly what would hide the two arms disagreeing about how many pairs they had produced. Two long collections were added to both tables. |
| 2026-09-16 | **A second poison that did not fire, recorded rather than dropped.** Swapping the order in which a value is tried as a scalar and as a collection changes nothing in 3,089 lines. The scanners are disjoint on their first byte and neither moves the cursor on failure, so the order is free — and the comment claiming it mattered was simply wrong. A unit test now pins both halves, because an accidental property nobody checks is one edit away from being false. Same for the two narrowed sub-slices in `multiSearch` and `search`: indexing them unchecked panics nowhere, so those bounds are defence in depth like `at()`'s. |
| 2026-09-16 | **The crates.io description was corrected, not left aspirational.** It claimed "the query-by-path search", which is not written. A published description is a capability claim and falls under the same discipline as the README. |
