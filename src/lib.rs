//! # gregex
//!
//! **gregex** is a regular expression library, providing multiple regex engines under a unified API.
//!
//! ## Features
//!
//! - **Multiple Engines:** Choose between a Pike VM interpreter and a JIT-compiled Pike VM engine for regex matching.
//! - **Compatibility:** Designed to be consistent with the [`regex`](https://docs.rs/regex) crate,.
//!
//! ## Usage
//!
//! ```rust
//! use gregex::Regex;
//!
//! let re = Regex::pike_vm(r"\d+").unwrap();
//! assert!(re.is_match("abc123"));
//! let mat = re.find("abc123").unwrap();
//! assert_eq!(mat.as_str(), "123");
//! ```
//!
//! ## Engines
//!
//! - [`thompson::pike_vm`] — Interpreted Pike VM engine.
//! - [`thompson::pike_jit`] — JIT-compiled Pike VM engine.
//!
//! ## Crate Organization
//!
//! - `regex`: Core API and engine dispatch
//! - `thompson`: Engine implementations based on thompson's constrcution
//! - `util`: Shared types and helpers
//!
//! ## License
//!
//! Licensed under MIT or Apache-2.0.

pub mod regex;
pub mod thompson;
pub mod util;

pub use regex::Builder;
pub use regex::Regex;
