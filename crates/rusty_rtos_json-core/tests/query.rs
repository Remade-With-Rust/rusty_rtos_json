//! `search` and `iterate` against `JSON_SearchConst` and `JSON_Iterate`.
//!
//! The C arm is `oracle/search_driver.c` driving `core_json.c` compiled
//! VERBATIM from the pinned checkout (v3.3.1 at `cffa492`), and its trace is
//! checked in, so this runs in CI with no C toolchain — the same arrangement
//! the kernel corpus, the three heap differentials and `backoff` use.
//!
//! **Our arm regenerates the trace rather than parsing it.** Building the same
//! text from our own side and diffing line by line is how the `backoff`
//! differential works, and it has a property that a parse-and-compare loop
//! does not: a line we fail to produce at all is a divergence, where a parser
//! would simply not check it.
//!
//! # The three workloads, and why each is here
//!
//! 1. **30 documents × 39 queries.** The documents carry the collisions
//!    between JSON and the *query grammar* — a key containing the separator,
//!    a key containing a bracket, an empty key. The queries carry the ways a
//!    query is malformed — an empty part, a trailing separator, a doubled
//!    separator, an unclosed bracket, an index at and past `u32::MAX`.
//!
//! 2. **Both entry points over all 318 corpus files.** 188 are malformed on
//!    purpose, and neither API validates first: they walk whatever they are
//!    handed. That is precisely where a reimplementation reads off the end,
//!    so the adversarial corpus earns its keep a second time here.
//!
//! 3. **`iterate` driven to exhaustion**, so the *final status* is compared
//!    and not merely the pairs before it.

// A test asserts; the workspace's deny-by-default is written for library code
// where a panic is a defect.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;
use std::path::Path;

use rusty_rtos_json_core::search::{Kind, NotFound, Stopped, iterate, search};

/// The C driver's document table, in the same order.
///
/// Two of these documents carry a backslash escape, and both are assembled
/// with an explicit `\x5c` rather than spelled out. An escape is only ASCII
/// bytes in a JSON document, and writing one literally here is how a sibling
/// test file once ended up with a real accented character inside a raw byte
/// string, which does not compile.
const DOCS: [&[u8]; 30] = [
    br#"{"a":1}"#,
    br#"{"a":{"b":{"c":42}}}"#,
    br#"{"a":[1,2,3]}"#,
    br#"[1,2,3]"#,
    br#"[[1,2],[3,4]]"#,
    br#"{"a":[{"b":1},{"b":2}]}"#,
    br#"{"x":"hello"}"#,
    br#"{"x":""}"#,
    b"{\"x\":\"a\x5c\"b\"}",
    b"{\"x\":\"\x5cu00e9\"}",
    br#"{"a.b":1}"#,
    br#"{"a[0]":1}"#,
    br#"{"":1}"#,
    br#"{"a":true,"b":false,"c":null}"#,
    br#"{"a":-1.5e10}"#,
    br#"{ "a" : 1 , "b" : 2 }"#,
    br#"{"a":1,"a":2}"#,
    br#"[]"#,
    br#"{}"#,
    br#"{"a":[]}"#,
    br#"{"a":{}}"#,
    br#"[{"a":1},{"b":2}]"#,
    br#"{"a":[[["deep"]]]}"#,
    br#"{"big":[0,1,2,3,4,5,6,7,8,9]}"#,
    b"",
    b"   ",
    br#""bare""#,
    br#"42"#,
    // Long enough to drive the iteration loop past the 32-pair cap.
    br#"[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39]"#,
    br#"{"k0":0,"k1":1,"k2":2,"k3":3,"k4":4,"k5":5,"k6":6,"k7":7,"k8":8,"k9":9,"k10":10,"k11":11}"#,
];

/// The C driver's query table, in the same order.
const QUERIES: [&[u8]; 39] = [
    b"a",
    b"b",
    b"x",
    b"c",
    b"big",
    b"a.b",
    b"a.b.c",
    b"a.b.c.d",
    b"",
    b".",
    b"..",
    b"a.",
    b".a",
    b"a..b",
    b"[0]",
    b"[1]",
    b"[2]",
    b"[3]",
    b"[10]",
    b"[4294967295]",
    b"[4294967296]",
    b"[99999999999999999999]",
    b"a[0]",
    b"a[1]",
    b"a[3]",
    b"a[0].b",
    b"a.b[0]",
    b"[0][0]",
    b"[0][1]",
    b"[1][0]",
    b"a[0][0][0]",
    b"big[9]",
    b"big[10]",
    b"a.b]",
    b"a[",
    b"a[]",
    b"a[-1]",
    b"a.b.",
    b"a[0]b",
];

/// The C driver's cap on pairs reported per document.
const CAP: usize = 32;

/// The C's `JSONStatus_t` names, which is what the trace speaks.
///
/// Our two refusal types are richer than the C's single enum only in that
/// they cannot express the impossible variant; mapping back to its names is
/// what makes the two arms comparable at all.
const fn search_status(
    r: &Result<rusty_rtos_json_core::search::Match<'_>, NotFound>,
) -> &'static str {
    match r {
        Ok(_) => "Success",
        Err(NotFound::Missing) => "NotFound",
        Err(NotFound::BadQuery) => "BadParameter",
    }
}

const fn stopped_status(e: Stopped) -> &'static str {
    match e {
        Stopped::Exhausted => "NotFound",
        Stopped::NotACollection => "IllegalDocument",
        Stopped::BadCursor => "BadParameter",
    }
}

const fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::String => "String",
        Kind::Number => "Number",
        Kind::True => "True",
        Kind::False => "False",
        Kind::Null => "Null",
        Kind::Object => "Object",
        Kind::Array => "Array",
    }
}

/// One search, in the driver's line format.
fn one_search(out: &mut String, tag: &str, buf: &[u8], query: &[u8]) {
    let r = search(buf, query);
    let status = search_status(&r);

    match r {
        Ok(m) => {
            let _ = writeln!(
                out,
                "{tag} {status} {} {} {}",
                m.offset,
                m.value.len(),
                kind_name(m.kind)
            );
        }
        // On a refusal the C leaves its out-parameters untouched, so printing
        // them would be printing the driver's own initialisers on both sides
        // and proving nothing.
        Err(_) => {
            let _ = writeln!(out, "{tag} {status}");
        }
    }
}

/// Every pair of a collection, in the driver's line format.
fn all_pairs(out: &mut String, tag: &str, buf: &[u8]) {
    let (mut start, mut next) = (0usize, 0usize);
    let mut n = 0usize;

    loop {
        match iterate(buf, &mut start, &mut next) {
            Err(e) => {
                let _ = writeln!(out, "{tag} end {}", stopped_status(e));
                return;
            }
            Ok(pair) => {
                let _ = write!(out, "{tag} pair {n} ");

                match pair.key {
                    None => {
                        let _ = write!(out, "nokey ");
                    }
                    Some(key) => {
                        let _ = write!(out, "key {} {} ", pair.key_offset, key.len());
                    }
                }

                let _ = writeln!(
                    out,
                    "{} {} {}",
                    pair.offset,
                    pair.value.len(),
                    kind_name(pair.kind)
                );
            }
        }

        n = n.saturating_add(1);

        if n >= CAP {
            let _ = writeln!(out, "{tag} capped");
            return;
        }
    }
}

fn corpus() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../oracle/test_parsing"
    ))
}

/// The corpus file names in the order the driver saw them: `LC_ALL=C ls`,
/// which is a byte-wise sort.
fn corpus_files() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(corpus())
        .expect("the corpus is vendored")
        .map(|e| {
            e.expect("a readable entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| n.ends_with(".json"))
        .collect();
    names.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    names
}

/// Build our side of the trace, in the driver's exact format.
fn our_trace() -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "geometry docs={} queries={} cap={CAP}",
        DOCS.len(),
        QUERIES.len()
    );

    for (d, doc) in DOCS.iter().enumerate() {
        let _ = writeln!(out, "doc {d} {}", doc.len());

        for (q, query) in QUERIES.iter().enumerate() {
            one_search(&mut out, &format!("search {d} {q}"), doc, query);
        }

        all_pairs(&mut out, &format!("iter {d}"), doc);
    }

    for name in corpus_files() {
        let bytes = std::fs::read(corpus().join(&name)).expect("the corpus is complete");
        let _ = writeln!(out, "file {name} {}", bytes.len());

        for query in ["a", "[0]", "a.b"] {
            one_search(
                &mut out,
                &format!("fsearch {name} {query}"),
                &bytes,
                query.as_bytes(),
            );
        }

        all_pairs(&mut out, &format!("fiter {name}"), &bytes);
    }

    let _ = writeln!(out, "end");
    out
}

#[test]
fn our_query_engine_matches_the_c_call_for_call() {
    let theirs = include_str!("../../../oracle/search.trace");
    let ours = our_trace();

    let mut their_lines = theirs.lines();
    let mut our_lines = ours.lines();
    let mut n = 0usize;

    loop {
        let (t, o) = (their_lines.next(), our_lines.next());

        match (t, o) {
            (None, None) => break,
            (Some(t), Some(o)) => {
                assert_eq!(o, t, "line {n} diverged\n  the C: {t:?}\n  ours : {o:?}");
            }
            (Some(t), None) => panic!("our trace ran out at line {n}; the C still has {t:?}"),
            (None, Some(o)) => panic!("the C trace ran out at line {n}; we still have {o:?}"),
        }

        n = n.saturating_add(1);
    }

    assert_eq!(n, 3089, "the trace should be 3,089 lines, not {n}");
}

/// The guard: the workload must actually reach every branch worth reaching.
///
/// heap_4's guard fails on too few refusals, heap_1's on never exhausting,
/// heap_5's on an unvisited region, backoff's on an unvisited branch and the
/// validator's on a corpus that stops being mostly rejections. This is the
/// fifth shape, and it says the same thing: a differential whose workload
/// cannot fail is a differential about nothing.
#[test]
fn the_workload_reaches_every_outcome() {
    let (mut success, mut missing, mut bad) = (0u32, 0u32, 0u32);
    let mut kinds = [0u32; 7];

    for doc in DOCS {
        for query in QUERIES {
            match search(doc, query) {
                Ok(m) => {
                    success += 1;
                    let slot = match m.kind {
                        Kind::String => 0,
                        Kind::Number => 1,
                        Kind::True => 2,
                        Kind::False => 3,
                        Kind::Null => 4,
                        Kind::Object => 5,
                        Kind::Array => 6,
                    };
                    if let Some(n) = kinds.get_mut(slot) {
                        *n += 1;
                    }
                }
                Err(NotFound::Missing) => missing += 1,
                Err(NotFound::BadQuery) => bad += 1,
            }
        }
    }

    assert!(success > 20, "too few successful queries: {success}");
    assert!(missing > 100, "too few honest misses: {missing}");
    assert!(bad > 100, "too few malformed queries refused: {bad}");

    // Every one of the seven types must be produced by something, or the
    // type mapping is only half tested.
    for (i, n) in kinds.iter().enumerate() {
        assert!(*n > 0, "no query ever returned type {i}");
    }

    // And every refusal an iterate can give must be reached.
    let (mut exhausted, mut illegal, mut bad_cursor) = (0u32, 0u32, 0u32);
    for doc in DOCS {
        let (mut start, mut next) = (0usize, 0usize);
        loop {
            match iterate(doc, &mut start, &mut next) {
                Ok(_) => {}
                Err(Stopped::Exhausted) => {
                    exhausted += 1;
                    break;
                }
                Err(Stopped::NotACollection) => {
                    illegal += 1;
                    break;
                }
                Err(Stopped::BadCursor) => {
                    bad_cursor += 1;
                    break;
                }
            }
        }
    }
    assert!(exhausted > 0, "no collection was ever exhausted");
    assert!(illegal > 0, "no non-collection was ever refused");
    assert!(bad_cursor > 0, "no bad cursor was ever refused");
}
