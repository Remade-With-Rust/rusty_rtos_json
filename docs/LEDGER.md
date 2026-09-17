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

## Conformance (2026-09-16) — the query engine

| quantity | value | method |
|---|---|---|
| trace lines agreeing with the C | **3,089 / 3,089** | `cargo test -p rusty_rtos_json-core --test query`. Our arm REGENERATES the trace in the driver's format and diffs it line for line, rather than parsing it — so a line we fail to produce at all is a divergence, which a parse-and-compare loop would simply not check. C arm: `oracle/search_driver.c` driving `core_json.c` compiled verbatim from v3.3.1 at `cffa492`. |
| queries compared | **2,124** | 30 documents x 39 queries (1,170) plus 3 queries over each of the 318 corpus files (954). Each compares status, value offset, value length and type. |
| of which succeeded / missed / were refused | **202 / 1,575 / 347** | the refusals are the point: a query engine that only answers the easy questions agrees about nothing. |
| types returned | **all 7** | Number 102, String 71, Array 10, Object 8, True 4, False 4, Null 3. A standing test fails if any type is never produced. |
| collections iterated to a verdict | **348** | 30 documents and 318 corpus files, each driven until `iterate` refuses, so the FINAL status is compared and not only the pairs before it. 267 pairs in total. |
| successful queries against MALFORMED documents | **70** | neither entry point validates first, and neither does the C. Those 70 answers came out of files JSONTestSuite marks `n_`, and every one had to match. |
| poison rows | **5 caught, 1 not** | caught: quote-stripping, the trailing-separator `- 1`, first-duplicate-key-wins, the huge-index latch to -1, the NULL key for an array element. Not caught: the scanner order in `nextValue`, which is genuinely free — see the plan's decision log. |

## The no-panic gate (2026-09-16)

| quantity | value | method |
|---|---|---|
| documents fed in with no panic | **22,085** | `cargo test -p rusty_rtos_json-core --test no_panic`, counted from the literals: 256 unstructured (every length 0..256) + 20,000 JSON-alphabet (deterministic LCG, seeds 1 and 2) + 202 truncations (6 valid documents of 35/36/38/36/26/25 bytes, cut at every offset including both ends) + 1,610 corruptions (46 positions × 35 interesting bytes) + 15 over-deep (5 depths × 3 shapes) + 2 corpus stress files. |
| deliberate panics introduced | **3** | 2 were caught (unchecked slice in the literal scanner; unchecked index into the depth stack). |
| deliberate panics NOT caught | **1** | making the universal byte reader `at()` index instead of `get()` left all 11 tests passing — so no scanner reads past the end. Recorded as a property of the transcription, not as a hole in the gate; the note lives next to the function. |
| documents fed through the QUERY surface | **50,920** | 20,000 random documents x random queries (LCG seed 3) + 20,000 iterations driven to exhaustion (seed 4) + 368 searches and 46 iterations over every truncation of a 45-byte nested document + 8,280 single-byte corruptions of it + 1,908 searches and 318 iterations over the corpus. |
| **both halves** | **73,005** | and every one deterministic: the pseudo-random arms use a written-out LCG, not a system source. |
| liveness | **asserted, not assumed** | `iterate` carries a caller-owned cursor, so one that stops advancing HANGS rather than failing, and a hang is reported as "still running". A test asserts the cursor strictly advances on every success. Stalling it on purpose produced a named assertion with the offending document printed, in place of an infinite loop. |
| deliberate panics in the query half | **3 introduced, 1 caught** | the stalled cursor was caught by the liveness test. Taking `multiSearch`'s narrowed sub-slice unchecked, and taking `search`'s returned slice unchecked, both left every test passing — the narrowing arithmetic only ever shrinks, so like `at()`'s bound these are defence in depth rather than load-bearing. |

## The build fact (2026-09-16)

| gate | result |
|---|---|
| `cargo test -p rusty_rtos_json-core` | 20 passed, 0 failed (5 differential, 10 no-panic, 2 query, 1 unit, 2 doc) |
| `cargo clippy --all-targets --all-features` under the workspace lint policy | clean, 0 warnings |
| `cargo build -p rusty_rtos_json --no-default-features --target thumbv7em-none-eabihf` | passes |
| `cargo build -p rusty_rtos_json --no-default-features --target riscv32imac-unknown-none-elf` | passes |

No speed number and no size number: neither half has been benchmarked, and
nothing here has run on a chip.
