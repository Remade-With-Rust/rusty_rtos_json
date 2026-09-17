# rusty_rtos_json — the ledger

Every number this package claims, with the run that produced it. A row
without a method is not a number. Counters before clocks; an external oracle
before a self-metric; the method line names the machine, the pinning, the arm
order and the null-arm floor for anything timed.

## Conformance (2026-09-16) — the validator

A count is a number and belongs here, with its method, exactly as a timing
would. These are the numbers the README quotes.

| quantity | value | method |
|---|---|---|
| files where our verdict equals coreJSON's | **318 / 318** | `cargo test -p rusty_rtos_json-core --test jsontestsuite`. Our arm: `validate()`. C arm: `JSON_Validate` from `core_json.c`, compiled verbatim from the pinned checkout (v3.3.1 at `cffa492`), captured once into `oracle/corejson.trace` as `<name> <0 accepted \| 1 rejected>`. |
| JSONTestSuite `y_` accepted | **95 / 95** | same run. The suite's own verdict, not the C's — a separate test, because it is a separate claim. |
| JSONTestSuite `n_` rejected | **188 / 188** | same run. |
| `i_` (implementation-defined) | **35 seen**, 10 accepted / 25 rejected by coreJSON and by us | the suite has no opinion on these; only the differential does. |
| corpus pin | `nst/JSONTestSuite` @ `1ef36fa` (2024-11-22) | vendored into `oracle/test_parsing/`, MIT, licence alongside. The file counts are asserted, so a corpus that moved under us fails rather than re-scoring. |

**The C was measured against the corpus first.** coreJSON scores 100 %, which
is the only reason "agree with the C" and "pass the suite" are the same target
here. Had it failed anywhere, one of the two would have had to give.

**Poison rows.** Four behaviours were broken on purpose to prove the
differential can fail: over-long UTF-8 (fails 2 files), lone surrogates (3),
trailing commas (2), leading zeros (3).

## The no-panic gate (2026-09-16)

| quantity | value | method |
|---|---|---|
| documents fed in with no panic | **22,085** | `cargo test -p rusty_rtos_json-core --test no_panic`, counted from the literals: 256 unstructured (every length 0..256) + 20,000 JSON-alphabet (deterministic LCG, seeds 1 and 2) + 202 truncations (6 valid documents of 35/36/38/36/26/25 bytes, cut at every offset including both ends) + 1,610 corruptions (46 positions × 35 interesting bytes) + 15 over-deep (5 depths × 3 shapes) + 2 corpus stress files. |
| deliberate panics introduced | **3** | 2 were caught (unchecked slice in the literal scanner; unchecked index into the depth stack). |
| deliberate panics NOT caught | **1** | making the universal byte reader `at()` index instead of `get()` left all 11 tests passing — so no scanner reads past the end. Recorded as a property of the transcription, not as a hole in the gate; the note lives next to the function. |

## The build fact (2026-09-16)

| gate | result |
|---|---|
| `cargo test -p rusty_rtos_json-core` | 11 passed, 0 failed |
| `cargo clippy --all-targets --all-features` under the workspace lint policy | clean, 0 warnings |
| `cargo build -p rusty_rtos_json --no-default-features --target thumbv7em-none-eabihf` | passes |
| `cargo build -p rusty_rtos_json --no-default-features --target riscv32imac-unknown-none-elf` | passes |

No speed number and no size number: the validator has not been benchmarked, and
nothing here has run on a chip.
