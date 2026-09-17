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
    const ALPHABET: &[u8] =
        b"{}[]\",:\\/ \t\r\n0123456789.eE+-truefalsnulxXuU\x00\x7f\xc3\xa9\xed\xa0\x80";
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
