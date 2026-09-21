# Threat model — `rusty_rtos_json`

**Unit tier:** critical-path. **Model version:** 1, 2026-09-21.
**Scope:** the JSON parser and its query path — a remake of AWS's coreJSON.

Satisfies `use-protection-please` **H-01**. The secrets position is §4
(**H-20**); the residual-risk register is §6 (**H-41**).

> This model is written for **this** unit. The kernel's model covers the
> scheduler and says so; an earlier version of this file was the kernel's
> plan unedited, which described "a kernel with no network stack" in the
> document belonging to the package whose entire job is reading bytes off
> one. The threats below are a parser's.

---

## 1. What this unit is

A `no_std`, allocation-free JSON reader. It takes **a byte slice from
somewhere untrusted** — a wire, a store, a bus — and answers questions about
it. It never writes JSON, never executes anything it reads, and holds no
state between calls.

That shape decides everything: this unit has exactly one adversary and one
input, and the entire model is about what that input can do.

---

## 2. Assets and adversary

| asset | failure |
|---|---|
| **Availability of the firmware** | a panic here is a halt — in `no_std` there is nothing to catch it, so one malformed document takes down the device |
| **Memory safety of the caller** | a slice handed back that outlives or overruns its input |
| **Answer integrity** | a document read as meaning something it does not, so the caller acts on a value the sender never sent |

**The adversary is whoever controls the bytes.** Not a co-resident task, not
a supply-chain actor in the first instance — the person on the other end of
the connection. They choose every byte, the length, the nesting depth, the
encoding, and where the input is cut off.

They can send: valid JSON; JSON that is valid but enormous; JSON nested a
million deep; bytes that are not JSON at all; **a valid document truncated at
any offset**; a valid document with one byte changed; and anything at all
that happens to look like a prefix of something valid.

---

## 3. The attack paths, and the evidence against each

### 3.1 A panic on crafted input — the whole game

This is the highest-value path by a distance, because the payoff is the
device and the cost is one packet.

*Mitigation:* the crate is `#![forbid(unsafe_code)]`, and indexing, `unwrap`,
`expect`, `panic` and unchecked arithmetic are lint-denied, so every fallible
path returns a `Result` the caller must read.

*Evidence, and it is the part that matters:* ten property tests drive the
public surface with adversarial input rather than with examples —

- arbitrary bytes;
- JSON-shaped noise, which is the harder case because it gets further in;
- **every truncation of a valid document**;
- **every single-byte corruption** of a valid document;
- nesting past the limit, which must be *refused* rather than fatal;
- the same four again through the **query** path, which is the one a caller
  actually uses.

"Every truncation" and "every single-byte corruption" are the two that catch
real defects, because a parser's bugs cluster at the point where it runs out
of input or meets a byte it did not expect in a state it does not handle.

### 3.2 Reading a document as meaning the wrong thing

A parser that accepts what it should reject is an integrity failure, not a
robustness one, and no amount of not-panicking detects it.

*Mitigation:* the unit is diffed against **coreJSON**, the C implementation
it remakes, over **all 318 files of JSONTestSuite** — including the `y_`
(must accept) and `n_` (must reject) sets. Agreement is on the verdict, file
by file, so a document either implementation treats differently is a failure
here.

*Why that is stronger than a test suite we wrote:* JSONTestSuite exists
specifically to find where parsers disagree, and the oracle is the parser the
caller would otherwise have used.

### 3.3 Resource exhaustion without a panic

*Mitigation:* no allocation at all — the reader borrows the caller's slice
and keeps a bounded cursor — and nesting is bounded by a limit that refuses
rather than recurses. `iterating_arbitrary_documents_always_terminates` pins
the property that matters: an adversary cannot make a loop that does not end.

---

## 4. Secrets — H-20

**No secret enters this unit.** It parses documents; it does not hold keys,
credentials or tokens, and it does not log. A document *may itself contain* a
secret the caller cares about — a token in a JSON body — and this unit's
obligation there is precise and limited:

- it never copies a value out of the caller's buffer (it hands back slices),
  so it creates no second copy to zeroize;
- it never logs, formats or traces document content, so a value cannot reach
  a UART because this unit put it there.

Zeroizing the caller's buffer is the caller's business, and this unit cannot
do it without owning memory it deliberately does not own.

---

## 5. Assumptions

1. **The caller's slice stays valid for the borrow.** Enforced by the borrow
   checker, not by convention.
2. **coreJSON is correct enough to be an oracle.** Where it and JSONTestSuite
   disagree, JSONTestSuite is the authority and the disagreement is a finding.
3. **The caller reads the `Result`.** A refused document that is treated as
   an empty one is the caller's defect, and the API is shaped to make it
   awkward.

---

## 6. Residual risks — H-41

| # | residual risk | severity | why accepted | closes when |
|---|---|---|---|---|
| R-1 | **No continuous fuzzing** (H-27). The property tests are bounded runs in CI, not a fuzzer that keeps going. | medium | the bounded suite covers the two classes that find parser bugs — every truncation and every single-byte corruption — over a real corpus rather than random noise | a `cargo fuzz` target runs continuously with no open crashes |
| R-2 | **No `cargo fuzz` target** (H-26). The no-panic suite is a property test, not a fuzz target, and the gate names the latter. | medium | the suite drives the same entry points with adversarial input and is *reproducible*, which a fuzzer is not; it is a floor, not a substitute | a fuzz target exists per public entry point |
| R-3 | **`cargo vet` coverage not established** (H-10). | low | the dependency set is essentially empty — this unit is `no_std` with no runtime dependencies | a `supply-chain/` config exists and the fleet gate runs it |
| R-4 | **Agreement is proved against ONE oracle.** coreJSON and JSONTestSuite agree here; a third implementation might not. | low | those two are the ones a caller would otherwise use | another oracle is added and disagrees |
| R-5 | **Threat model not revisited after a major change** (H-02). Version 1. | low | no major change since writing | the next change to the parser's input handling |

---

## 7. What would change this model

A write path. The moment this unit *emits* JSON rather than only reading it,
the adversary gains a second surface — an application value that reaches a
wire — and §4's "it never copies a value out" stops being true. That is the
change to watch for, and it is not on the roadmap today.
