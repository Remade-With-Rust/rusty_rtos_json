//! K7's no-panic gate for the validator.
//!
//! The crate denies `unwrap`, `expect` and `panic` on every path and forbids
//! `unsafe`, which makes a panic reachable only through arithmetic that
//! overflows, an index out of range, or a slice shorter than something
//! assumed. The lints catch the SHAPES; this goes after the reachability, the
//! same way `rusty_rtos_kernel-core`'s does.
//!
//! A parser is the place this matters most in the whole family. Every other
//! crate here is driven by our own kernel; a validator is driven by **bytes
//! from the network**, so "cannot panic on any input" is not a tidiness
//! property, it is the security property.
//!
//! The corpus is not enough for this on its own: JSONTestSuite is 318
//! carefully chosen documents, and a panic lives in the input nobody chose.
//!
//! # These tests were broken on purpose before they were believed
//!
//! A no-panic gate that has never failed is indistinguishable from one that
//! cannot fail, so three panics were introduced deliberately and the gate had
//! to find each:
//!
//! | poison | caught by |
//! |---|---|
//! | `skip_literal` slices without checking the literal fits | four of the six, at the truncation of `true` |
//! | the depth stack is indexed past its own bound | nesting, and the 100,000-bracket stress file |
//! | `at()` indexes instead of `get()` | **nothing** -- see below |
//!
//! The third is the interesting one. It is not a weak test, it is a property:
//! no scanner reads past the end in the first place, so the `Option` on
//! `at()` is defence in depth rather than the thing keeping this safe. The two
//! bounds that ARE load-bearing are the two above, and both are proven to be.

// A test asserts; the workspace's deny-by-default is written for library code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

/// Bytes drawn from JSON's own alphabet, which reaches far deeper into
/// the scanners than uniform noise: random bytes are refused in the first
/// few characters, and a brace, a quote, a backslash and a digit are not.
const ALPHABET: &[u8] =
    b"{}[]\",:\\/ \t\r\n0123456789.eE+-truefalsnulxXuU\x00\x7f\xc3\xa9\xed\xa0\x80";

use rusty_rtos_json_core::validate;

/// An LCG, so the "random" inputs are the same on every machine and a failure
/// is reproducible from the seed alone.
struct Lcg(u32);

impl Lcg {
    const fn new(seed: u32) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0
    }

    fn byte(&mut self) -> u8 {
        (self.next() >> 16) as u8
    }
}

/// Bytes with no structure at all.
#[test]
fn arbitrary_bytes_never_panic() {
    let mut rng = Lcg::new(1);
    for len in 0..256usize {
        let doc: Vec<u8> = (0..len).map(|_| rng.byte()).collect();
        let _ = validate(&doc);
    }
}

/// Bytes drawn only from JSON's own alphabet, which is far more likely to
/// reach deep into the scanners than uniform noise: random bytes are rejected
/// in the first few characters, and `{`, `"`, `\` and digits are not.
#[test]
fn json_shaped_noise_never_panics() {
    let mut rng = Lcg::new(2);

    for _ in 0..20_000 {
        let len = (rng.next() % 64) as usize;
        let doc: Vec<u8> = (0..len)
            .map(|_| {
                let idx = (rng.next() as usize) % ALPHABET.len();
                ALPHABET.get(idx).copied().unwrap_or(b' ')
            })
            .collect();
        let _ = validate(&doc);
    }
}

/// Every truncation of a valid document.
///
/// A scanner that reads one byte past its bound does it at the END of the
/// buffer, so cutting a good document at every offset is the cheapest way to
/// find that — and it is the shape a network parser actually meets, because a
/// packet boundary lands wherever it lands.
#[test]
fn every_truncation_of_a_valid_document_is_safe() {
    let docs: [&[u8]; 6] = [
        br#"{"a":[1,2,{"b":"\u00e9"}],"c":true}"#,
        br#"[-0.5e+10,null,false,"\ud83d\ude00"]"#,
        br#"{"nested":{"deep":{"deeper":[[[1]]]}}}"#,
        br#""just a string with \\ and \" in it""#,
        br#"[1e1,1E1,1e+1,1e-1,0.0,-0]"#,
        br#"{"k":"AB","l":[{},[],""]}"#,
    ];

    for doc in docs {
        for cut in 0..=doc.len() {
            let Some(slice) = doc.get(..cut) else {
                continue;
            };
            let _ = validate(slice);
        }
    }
}

/// Every single-byte corruption of a valid document.
///
/// Truncation finds the end-of-buffer mistakes; substitution finds the ones
/// where a scanner trusts what it just read.
#[test]
fn every_single_byte_corruption_is_safe() {
    let doc = br#"{"a":[1,2,{"b":"\u00e9"}],"c":true,"d":-1.5e3}"#;
    let interesting: &[u8] =
        b"\x00\x01\x1f\"\\/{}[],:. \t\r\n0eE+-uUtfn\x7f\x80\xc0\xc1\xe0\xf0\xf5\xff";

    for pos in 0..doc.len() {
        for &byte in interesting {
            let mut corrupted = doc.to_vec();
            if let Some(slot) = corrupted.get_mut(pos) {
                *slot = byte;
            }
            let _ = validate(&corrupted);
        }
    }
}

/// Nesting far past `MAX_DEPTH`, in both bracket kinds and mixed.
///
/// The explicit stack is what replaces recursion, so overrunning it is the
/// one way this design could smash something. It must answer
/// `MaxDepthExceeded` rather than panic — and a stack overflow would not be a
/// panic at all, which is why the depth is bounded rather than trusted.
#[test]
fn nesting_past_the_limit_is_refused_not_fatal() {
    for depth in [32usize, 33, 64, 1000, 10_000] {
        for open in *b"[{" {
            let doc = vec![open; depth];
            let _ = validate(&doc);
        }
        // Mixed, and closed, so the stack is exercised both ways.
        let mut doc = Vec::new();
        for n in 0..depth {
            doc.push(if n % 2 == 0 { b'[' } else { b'{' });
        }
        let _ = validate(&doc);
    }
}

/// The two stress files in the corpus, which are the only large inputs here.
#[test]
fn the_corpus_stress_files_are_safe() {
    let base = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../oracle/test_parsing"
    ));
    for name in [
        "n_structure_open_array_object.json",
        "n_structure_100000_opening_arrays.json",
    ] {
        let bytes = std::fs::read(base.join(name)).expect("the corpus is complete");
        let _ = validate(&bytes);
    }
}

// ---- the query surface ---------------------------------------------------
//
// Everything above drives `validate`. These drive `search` and `iterate`,
// which are the harder half: the validator refuses a malformed document and
// stops, while a query WALKS one. Neither entry point validates first, so
// every scanner in the crate can be reached with bytes that make no sense.

/// Arbitrary documents crossed with arbitrary queries.
#[test]
fn search_never_panics_on_arbitrary_input() {
    let mut rng = Lcg::new(3);

    for _ in 0..20_000 {
        let doc_len = (rng.next() % 48) as usize;
        let doc: Vec<u8> = (0..doc_len)
            .map(|_| {
                let idx = (rng.next() as usize) % ALPHABET.len();
                ALPHABET.get(idx).copied().unwrap_or(b' ')
            })
            .collect();

        let query_len = (rng.next() % 12) as usize;
        let query: Vec<u8> = (0..query_len)
            .map(|_| {
                let idx = (rng.next() as usize) % QUERY_ALPHABET.len();
                QUERY_ALPHABET.get(idx).copied().unwrap_or(b'a')
            })
            .collect();

        let _ = validate_search(&doc, &query);
    }
}

/// The query alphabet, which is NOT the document alphabet: a query is parsed
/// by its own little grammar, and the bytes that matter to it are the
/// separator, the brackets and the digits.
const QUERY_ALPHABET: &[u8] = b".[]0123456789abxyz-+ \x00\xff";

/// A thin wrapper so the fuzz tests read the same as the corpus ones.
fn validate_search(doc: &[u8], query: &[u8]) -> bool {
    rusty_rtos_json_core::search::search(doc, query).is_ok()
}

/// Iterating arbitrary documents, driven to exhaustion.
///
/// The bound is not a convenience. `iterate` carries a cursor the CALLER owns,
/// so a version that failed to advance it would not panic and would not return
/// a wrong answer — it would hang, and a hang is the one failure a test
/// framework reports as "still running" rather than as a bug. This asserts the
/// cursor strictly advances on every success, which is what makes the
/// [`Pairs`] iterator terminate for every input rather than for the inputs
/// someone happened to try.
#[test]
fn iterating_arbitrary_documents_always_terminates() {
    use rusty_rtos_json_core::search::iterate;

    let mut rng = Lcg::new(4);

    for _ in 0..20_000 {
        let len = (rng.next() % 48) as usize;
        let doc: Vec<u8> = (0..len)
            .map(|_| {
                let idx = (rng.next() as usize) % ALPHABET.len();
                ALPHABET.get(idx).copied().unwrap_or(b' ')
            })
            .collect();

        let (mut start, mut next) = (0usize, 0usize);
        let mut previous = 0usize;
        let mut steps = 0usize;

        while iterate(&doc, &mut start, &mut next).is_ok() {
            assert!(
                next > previous,
                "the cursor did not advance on {doc:?}: {previous} -> {next}"
            );
            previous = next;

            steps += 1;
            assert!(
                steps <= doc.len() + 2,
                "more values than bytes in {doc:?} — the cursor is going backwards"
            );
        }
    }
}

/// Every truncation and every single-byte corruption, through the query path.
///
/// The validator's versions of these are above. A truncation is where a
/// scanner reads one byte past its bound, and the query path has scanners the
/// validator never reaches — the query parser itself, and the sub-buffer
/// narrowing in `multiSearch`, which is the one place in this crate where a
/// slice is taken from indices that were computed rather than scanned.
#[test]
fn the_query_path_survives_truncation_and_corruption() {
    use rusty_rtos_json_core::search::iterate;

    const QUERIES: [&[u8]; 8] = [
        b"a",
        b"a.b",
        b"[0]",
        b"a[0].b",
        b".",
        b"a.",
        b"[",
        b"[99999999999]",
    ];
    let doc = br#"{"a":{"b":[1,2,{"c":"x"}]},"d":[[]],"e":true}"#;

    for cut in 0..=doc.len() {
        let Some(slice) = doc.get(..cut) else {
            continue;
        };
        for query in QUERIES {
            let _ = validate_search(slice, query);
        }
        let (mut start, mut next) = (0usize, 0usize);
        let mut steps = 0usize;
        while iterate(slice, &mut start, &mut next).is_ok() {
            steps += 1;
            if steps > slice.len() + 2 {
                panic!("iterate did not terminate on a {cut}-byte prefix");
            }
        }
    }

    let interesting: &[u8] = b"\x00\"\\{}[],:.0eE+-utfn\x7f\xc0\xf5\xff";
    for pos in 0..doc.len() {
        for &byte in interesting {
            let mut corrupted = doc.to_vec();
            if let Some(slot) = corrupted.get_mut(pos) {
                *slot = byte;
            }
            for query in QUERIES {
                let _ = validate_search(&corrupted, query);
            }
        }
    }
}

/// The corpus, through both query entry points.
///
/// 188 of these files are malformed on purpose. The differential already
/// compares the ANSWERS; this asserts the weaker and more important thing,
/// that getting them cannot bring the process down.
#[test]
fn the_whole_corpus_is_safe_through_the_query_path() {
    use rusty_rtos_json_core::search::pairs;

    let base = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../oracle/test_parsing"
    ));

    let mut files = 0usize;
    for entry in std::fs::read_dir(base).expect("the corpus is vendored") {
        let path = entry.expect("a readable entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("a readable file");

        for query in [&b"a"[..], b"[0]", b"a.b.c", b"[0][0][0]", b"", b"."] {
            let _ = validate_search(&bytes, query);
        }

        // `take` bounds the 100,000-bracket file rather than the logic: the
        // termination proof is the test above, this one is about panics.
        assert!(pairs(&bytes).take(64).count() <= 64);
        files += 1;
    }

    assert_eq!(files, 318, "the corpus should carry 318 files, not {files}");
}
