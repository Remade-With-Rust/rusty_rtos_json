#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
//! `rusty_rtos_json` — coreJSON remade in Rust.
//!
//! **The validator.** `JSON_Validate`, strict ECMA-404. It agrees with coreJSON
//! v3.3.1 on all 318 files of JSONTestSuite and passes the suite outright
//! (95/95 accepted, 188/188 rejected).
//!
//! **The query engine.** `JSON_SearchConst` and `JSON_Iterate`, diffed against
//! the C over 2,124 queries and 348 iterations as a 3,089-line trace. A query
//! returns a sub-slice of the buffer you already have: no tree, no copy.
//!
//! Zero allocation throughout, `no_std`, `forbid(unsafe)`, with a no-panic gate
//! over both halves.
//!
//! **What is not here:** a serialiser, which coreJSON does not have either, and
//! `JSON_SearchT`, which is a cast of `JSON_SearchConst` that exists only so C
//! callers can pass a mutable buffer.
//!
//! This is the facade: it re-exports the `no_std` core. Depend on this crate;
//! reach into the sub-crates only when you are building a port or a backend.
//!
//! Part of Kairos (Remade With Rust). Plan: `docs/plans/rusty_rtos_json.md`.

pub use rusty_rtos_json_core::*;

/// The names a firmware wants in scope.
pub mod prelude {
    pub use rusty_rtos_json_core::prelude::*;
}
