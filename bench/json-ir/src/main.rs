//! Instruction counts for `JSON_Validate`, over the corpus it is gated on.
//!
//! The workload is the SAME 318 files the differential replays, so a count
//! taken here is a count of the work the gated path does -- and any change
//! that moves it has to leave all 318 verdicts identical.
//!
//! The corpus is read once, outside the loop; the loop is what the count is
//! about. `REPS` is large enough that process startup and the 318 reads are a
//! fraction of a percent of the total, so the number needs no cancelling.

use std::io::Read;

use rusty_rtos_json_core::is_valid;

/// Enough repetitions that startup and file I/O are noise in the total.
const REPS: u32 = 200;

fn corpus() -> Vec<Vec<u8>> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../oracle/test_parsing");
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {dir}: {e}"))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    // Sorted, so the sequence is the same on every machine and the checksum
    // is comparable across runs.
    entries.sort();

    entries
        .iter()
        .filter_map(|p| {
            let mut f = std::fs::File::open(p).ok()?;
            let mut v = Vec::new();
            f.read_to_end(&mut v).ok()?;
            Some(v)
        })
        .collect()
}

fn main() {
    let files = corpus();
    let bytes: usize = files.iter().map(Vec::len).sum();

    // A checksum so a compiler that removed the work is visible as a changed
    // number rather than as a suspiciously good count. It is the verdict of
    // every file, position-weighted, so a reordering shows up too.
    let mut checksum = 0u64;
    let mut accepted = 0u64;

    for _ in 0..REPS {
        for (i, buf) in files.iter().enumerate() {
            let ok = is_valid(buf);
            if ok {
                accepted = accepted.wrapping_add(1);
                checksum = checksum.wrapping_add(i as u64).wrapping_add(1);
            }
        }
    }

    println!("checksum {checksum}");
    println!(
        "files {} bytes {bytes} reps {REPS} validations {} accepted {}",
        files.len(),
        files.len() as u64 * u64::from(REPS),
        accepted / u64::from(REPS)
    );
}
