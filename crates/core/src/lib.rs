//! casefile-core: the deterministic "game engine" for burglary investigation.
//!
//! Design rule (Guy Ernest, AI-on-Cloud): LLMs propose, this crate verifies. Nothing here calls a
//! model, touches the network or the filesystem. Every function is pure and unit-tested, so an
//! output can be recreated exactly for a prosecutor or the defense.

#![forbid(unsafe_code)]

pub mod citations;
pub mod cost;
pub mod geo;
pub mod linkage;
pub mod property;
pub mod solvability;
pub mod types;
pub mod verify;

pub use types::*;

/// Look up a case by id.
pub fn find_case<'a>(ds: &'a Dataset, id: &str) -> Option<&'a Case> {
    ds.cases.iter().find(|c| c.id == id)
}

/// Engine version stamped into every audit record and packet (reproducibility).
pub const ENGINE_VERSION: &str = concat!("casefile-core ", env!("CARGO_PKG_VERSION"));
