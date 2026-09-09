#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
//! `rusty_rtos_json` — coreJSON remade in Rust: the strict ECMA-404 validator and the query-by-path search, zero allocation, no_std, forbid(unsafe), with the no-panic gate every parser in the family carries.
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
