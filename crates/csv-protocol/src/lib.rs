// SPDX-License-Identifier: Apache-2.0

//! Versioned worker protocol. See `frame` for bounded framing.

pub mod frame;
pub mod messages;

pub use frame::{read_message, write_message, FrameError};
pub use messages::{ClientMessage, ErrorCode, SourceSpec, WorkerMessage, PROTOCOL_VERSION};
