//! HashTask and Share types for work assignment and result reporting.
//!
//! This module re-exports from `crate::asic::hash_thread` for backwards
//! compatibility. New code should import from `asic::hash_thread` directly.

pub use crate::asic::hash_thread::{HashTask, Share};
