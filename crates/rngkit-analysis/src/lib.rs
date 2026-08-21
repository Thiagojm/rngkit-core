#![doc = include_str!("../README.md")]

pub mod accumulator;
pub mod error;

pub use accumulator::{Accumulator, Snapshot, analyze_records};
pub use error::AnalysisError;
