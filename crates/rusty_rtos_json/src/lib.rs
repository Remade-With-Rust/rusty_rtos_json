#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
//! `rusty_rtos_json` — coreJSON remade in Rust.
//!
//! **What is here:** `JSON_Validate`, the strict ECMA-404 validator. It agrees
//! with coreJSON v3.3.1 on all 318 files of JSONTestSuite and passes the suite
//! outright (95/95 accepted, 188/188 rejected). Zero allocation, `no_std`,
//! `forbid(unsafe)`, with a no-panic gate.
//!
//! **What is not:** `JSON_Search`, `JSON_SearchConst` and `JSON_Iterate`. The
//! query half of coreJSON is not written yet, and this crate does not pretend
//! otherwise.
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
