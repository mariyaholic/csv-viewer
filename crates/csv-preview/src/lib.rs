// SPDX-License-Identifier: Apache-2.0

//! Bounded preview pipeline and Windows Explorer preview handler.

pub mod backend;
pub mod registration;
pub mod session;
#[cfg(any(windows, feature = "windows-check"))]
pub mod windows;

pub use backend::{worker_beside_test_executable, WorkerBackend};
pub use session::{
    FullScanBackend, InProcessBackend, PreviewConfig, PreviewError, PreviewSession, SessionPhase,
    SessionSnapshot, SpoolProgress, SpoolSource,
};
