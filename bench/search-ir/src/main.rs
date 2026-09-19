//! Instruction counts for the JSON query engine.
//!
//! The other half of the crate: `JSON_SearchConst`, the dotted/indexed query
//! walk. The workload is a cross product of documents against queries, so hits,
//! misses, wrong-type rejections and malformed queries are all exercised in one
//! sweep rather than one at a time.
//!
//! A deterministic counter, not a clock. The three verdict counts are the work
//! parity anchors.

use rusty_rtos_json_core::search;

const REPS: u32 = 300;

const DOCS: &[&str] = &[
    r#"{"a":1,"b":{"c":2,"d":[3,4,5]},"e":"hello"}"#,
    r#"{"name":"kairos","ports":["cortex-m","riscv","xtensa"],"n":3}"#,
    r#"{"deep":{"deeper":{"deepest":{"leaf":42}}}}"#,
    r#"[10,20,30,40,50]"#,
    r#"{"x":[{"y":1},{"y":2},{"y":3}]}"#,
    r#"{"empty":{},"list":[],"nul":null,"t":true,"f":false}"#,
    r#"{"esc":"a\"b\\c","uni":"été"}"#,
    r#"{"a":{"a":{"a":{"a":{"a":1}}}}}"#,
    r#"{"num":-1.5e10,"zero":0,"big":268435455}"#,
    r#"{"dup":1,"dup":2}"#,
];

const QUERIES: &[&str] = &[
    "a",
    "b.c",
    "b.d[1]",
    "b.d[9]",
    "e",
    "name",
    "ports[2]",
    "deep.deeper.deepest.leaf",
    "[3]",
    "x[1].y",
    "empty",
    "nul",
    "esc",
    "a.a.a.a.a",
    "num",
    "missing",
    "b.missing.deeper",
    "dup",
    "",
    "a..b",
];

fn main() {
    let docs: Vec<&[u8]> = DOCS.iter().map(|s| s.as_bytes()).collect();
    let queries: Vec<&[u8]> = QUERIES.iter().map(|s| s.as_bytes()).collect();

    let mut found = 0u64;
    let mut missing = 0u64;
    let mut checksum = 0u64;

    for _ in 0..REPS {
        for (d, doc) in docs.iter().enumerate() {
            for (q, query) in queries.iter().enumerate() {
                match search(doc, query) {
                    Ok(m) => {
                        found = found.wrapping_add(1);
                        // The value's length folds in, so a walk that returned
                        // a different span shows as a changed checksum and not
                        // merely as the same verdict count.
                        checksum = checksum
                            .wrapping_add((d * queries.len() + q) as u64)
                            .wrapping_add(m.value.len() as u64);
                    }
                    Err(_) => missing = missing.wrapping_add(1),
                }
            }
        }
    }

    let pairs = (docs.len() * queries.len()) as u64;
    println!("checksum {checksum}");
    println!(
        "pairs {pairs} reps {REPS} calls {} found {} missing {}",
        pairs * u64::from(REPS),
        found / u64::from(REPS),
        missing / u64::from(REPS)
    );
}
