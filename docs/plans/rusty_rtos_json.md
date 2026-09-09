# rusty_rtos_json — package plan

**One sentence:** coreJSON remade in Rust: the strict ECMA-404 validator and the query-by-path search, zero allocation, no_std, forbid(unsafe), with the no-panic gate every parser in the family carries.

Family plan: Kairos `docs/plans/rtos-mission.md` (umbrella repo) — its §2.1
names what this package remakes, wraps and never touches; its §6 carries the
phase this package's kill test belongs to. This file obeys that one.

Written 2026-09-09. Status: **scaffold** — the crate layout, the feature ladder,
the lint policy and the CI gates exist; nothing is measured.

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

Nothing yet. The facade re-exports the core; the core exposes `VERSION`.

## 4. Roadmap

| Milestone | Adds | Driven by | Kill test |
|---|---|---|---|
| scaffold | the shape | K0 | a clean clone builds alone; CI green |

## 5. Deliberately absent

To be written with the first milestone.

## 6. Risks

| Risk | Mitigation |
|---|---|
| | |

## 7. Decision log

| Date | Decision |
|---|---|
| 2026-09-09 | Stamped from the Kairos template; obeys the family plan. |
| 2026-09-09 | **Relation to the house `rusty_json_turbo`** (serde_json forked and made fast; lib name `serde_json`; `no_std + alloc`, checked on the Kairos bare-metal targets in the umbrella's `tools/house-gate`): this package is the coreJSON *API* — the zero-allocation validator and `JSON_Search` over a byte buffer, `forbid(unsafe)`, no `alloc` — which serde_json's model (an `alloc`-backed `Value` and boxed errors) cannot provide; typed (de)serialization under `alloc` goes through `rusty_json_turbo` behind a `serde` feature, never a second parser for that job. The umbrella's `docs/HOUSE-STACK.md` carries the pin and the gate. |
