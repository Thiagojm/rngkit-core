#![doc = include_str!("../README.md")]

pub mod adapters;
pub mod config;
pub mod discovery;
pub mod error_mapping;
pub mod registry;

pub use config::SourceConfig;
pub use discovery::{DiscoveryIssue, DiscoveryReport, SourceCandidate, discover};
pub use registry::{OpenedSource, open};
