#![doc = include_str!("../README.md")]

pub mod adapters;
pub mod config;
pub mod error_mapping;
pub mod registry;

pub use config::SourceConfig;
pub use registry::{OpenedSource, open};
