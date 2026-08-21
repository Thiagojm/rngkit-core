#![doc = include_str!("../README.md")]

pub mod cancellation;
pub mod error;
pub mod event;
pub mod runner;

pub use cancellation::{CancelToken, Cancelled, Clock, FakeClock, StdClock};
pub use error::EngineError;
pub use event::{EngineEvent, EventSink, VecSink};
pub use runner::{
    EngineConfig, SessionOutcome, run_session, run_session_at, run_session_with_clock,
    with_writer_fail_point,
};
