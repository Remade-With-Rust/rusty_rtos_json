//! `validate` against `JSON_Validate`, on all 318 of JSONTestSuite.
//!
//! Two instruments in one file, and they answer different questions:
//!
//! * [`we_agree_with_corejson_on_every_file`] compares our verdict against
//!   **coreJSON's own**, file by file. That is the differential, and it is the
//!   one that can fail for a subtle reason.
//! * [`the_suite_is_passed_outright`] compares against **JSONTestSuite's
//!   verdict** — every `y_` accepted, every `n_` rejected. That is K7's
//!   headline claim.
//!
//! **They are the same target here, and that was measured rather than
//! assumed.** coreJSON scores 100 % on the suite: 95/95 accepted, 188/188
//! rejected. Had it failed anywhere, "agree with the C" and "pass the suite"
//! would have pulled apart and one would have had to give. Keeping both tests
//! means the day that changes, it is visible rather than silently resolved.
//!
//! The 35 `i_` files are where the standard leaves the answer to the
//! implementation, so the suite has no opinion and only the differential does.
//! coreJSON accepts 10 and rejects 25, and those 35 are where a transcription
//! drifts first — being *a* JSON parser does not determine them, being
//! *coreJSON* does.

// A test asserts; the workspace's deny-by-default is written for library code
// where a panic is a defect.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use rusty_rtos_json_core::{Validity, is_valid, validate};

/// coreJSON's verdict on every file, generated once and checked in:
/// `<name> <0 accepted | 1 rejected>`.
const ORACLE: &str = include_str!("../../../oracle/corejson.trace");

/// Where the vendored corpus lives.
fn corpus() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../oracle/test_parsing"
    ))
}

/// One row of the oracle: the file, and whether the C accepted it.
fn rows() -> impl Iterator<Item = (&'static str, bool)> {
    ORACLE.lines().filter_map(|line| {
        let (name, verdict) = line.rsplit_once(' ')?;
        Some((name, verdict.trim() == "0"))
    })
}

#[test]
fn we_agree_with_corejson_on_every_file() {
    let mut checked = 0usize;
    let mut disagreed = Vec::new();

    for (name, c_accepted) in rows() {
        let path = corpus().join(name);
        let bytes =
            std::fs::read(&path).unwrap_or_else(|e| panic!("the corpus is missing {name}: {e}"));

        let ours = is_valid(&bytes);
        if ours != c_accepted {
            disagreed.push(format!(
                "  {name}: the C {} it, we {} it  ({:?})",
                if c_accepted { "accepted" } else { "rejected" },
                if ours { "accept" } else { "reject" },
                validate(&bytes)
            ));
        }
        checked = checked.saturating_add(1);
    }

    assert_eq!(checked, 318, "the oracle has {checked} rows, not 318");
    assert!(
        disagreed.is_empty(),
        "{} of {checked} files disagree with coreJSON:\n{}",
        disagreed.len(),
        disagreed.join("\n")
    );
}

/// K7's headline: the suite at 100 %.
///
/// `y_` must be accepted and `n_` must be rejected. `i_` is
/// implementation-defined, so the suite has no opinion and this test does not
/// either — [`we_agree_with_corejson_on_every_file`] is what pins those.
#[test]
fn the_suite_is_passed_outright() {
    let (mut y_ok, mut y_bad, mut n_ok, mut n_bad, mut i_seen) = (0u32, 0u32, 0u32, 0u32, 0u32);

    for (name, _) in rows() {
        let bytes = std::fs::read(corpus().join(name)).expect("the corpus is complete");
        let ours = is_valid(&bytes);

        if name.starts_with("y_") {
            if ours {
                y_ok += 1;
            } else {
                y_bad += 1;
                eprintln!("MUST accept but rejected: {name} ({:?})", validate(&bytes));
            }
        } else if name.starts_with("n_") {
            if ours {
                n_bad += 1;
                eprintln!("MUST reject but accepted: {name}");
            } else {
                n_ok += 1;
            }
        } else {
            i_seen += 1;
        }
    }

    assert_eq!(
        y_bad, 0,
        "{y_bad} documents that must be accepted were rejected"
    );
    assert_eq!(
        n_bad, 0,
        "{n_bad} documents that must be rejected were accepted"
    );
    assert_eq!(y_ok, 95, "the corpus should carry 95 y_ files");
    assert_eq!(n_ok, 188, "the corpus should carry 188 n_ files");
    assert_eq!(i_seen, 35, "the corpus should carry 35 i_ files");
}

/// The guard: the corpus must actually exercise rejection.
///
/// For a parser the REJECTIONS are the hard half — accepting valid JSON is
/// what any half-written scanner does, and every `n_` file is a specific way
/// to be wrong. heap_4's guard fails on too few refusals, heap_1's on never
/// exhausting, backoff's on an unvisited branch; this one fails if the corpus
/// stops being mostly rejections.
#[test]
fn the_corpus_is_mostly_rejections() {
    let rejected = rows().filter(|(_, accepted)| !accepted).count();
    let total = rows().count();

    assert!(
        rejected * 2 > total,
        "only {rejected} of {total} files are rejected — the corpus has stopped \
         testing the half that is hard"
    );
}

/// The four kinds of "no" are distinguished, not collapsed.
///
/// A validator that answered a plain bool would pass the corpus and tell a
/// caller nothing. coreJSON reports depth exhaustion separately from a
/// malformed document because they need different fixes.
#[test]
fn the_reasons_for_refusal_are_distinct() {
    assert_eq!(validate(b""), Validity::BadParameter);
    assert_eq!(validate(b"{"), Validity::Partial);
    assert_eq!(validate(b"{]"), Validity::Illegal);
    assert_eq!(validate(b"[1,2]"), Validity::Valid);

    // 33 opening brackets: one past MAX_DEPTH.
    let deep = vec![b'['; 33];
    assert_eq!(validate(&deep), Validity::MaxDepthExceeded);
}

/// A scalar at the top level is a document, which is ECMA-404 and not RFC 4627.
///
/// coreJSON makes this switchable with `JSON_VALIDATE_COLLECTIONS_ONLY`; the
/// default accepts them, and so does this. Worth pinning because it is the
/// single most likely place for a reader to think the parser is too lax.
#[test]
fn a_bare_scalar_is_a_valid_document() {
    for doc in [
        &b"42"[..],
        b"-0.5e3",
        b"\"hello\"",
        b"true",
        b"false",
        b"null",
        b"  7  ",
    ] {
        assert_eq!(
            validate(doc),
            Validity::Valid,
            "{:?}",
            core::str::from_utf8(doc)
        );
    }
}
