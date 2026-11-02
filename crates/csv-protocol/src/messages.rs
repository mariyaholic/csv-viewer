// SPDX-License-Identifier: Apache-2.0

//! Versioned worker messages.
//!
//! The handler owns the source stream and spools it into a private directory.
//! The worker never receives an Explorer stream or a user-chosen path: it
//! receives a path inside the spool directory it was started with, plus the
//! expected byte count.

use csvview_core::{ScanProgress, ScanReport, ScanRequest};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpec {
    /// Absolute path inside the worker's `--spool-dir`.
    pub path: String,
    pub bytes: u64,
    /// The handler stopped copying before the source ended.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
// 07A-13: the `Scan` variant carries `SourceSpec` plus the scan request's
// limits, so it dominates the enum's size. Hardening it for lint (boxing the
// payloads) would complicate the serde wire format the worker protocol tests
// pin, for no real cost: at most one message is in flight per frame. The
// allow is deliberate, with the frame cap (`max_frame_bytes`) as the working
// bound on size.
#[allow(clippy::large_enum_variant)]
pub enum ClientMessage {
    Hello {
        protocol_version: u32,
    },
    Scan {
        id: u64,
        source: SourceSpec,
        request: ScanRequest,
    },
    Cancel {
        id: u64,
    },
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadVersion,
    BadRequest,
    Busy,
    SourceOutsideSpool,
    SourceSizeMismatch,
    SourceTooLarge,
    Cancelled,
    Io,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
// 07A-13: like `ClientMessage::Scan`, the `Result` variant carries the full
// `ScanReport` and dominates the enum's size. Reports are bounded and sent
// exactly once per scan; boxing them would obfuscate the round-trip tests
// for no gain, so the allow is deliberate.
#[allow(clippy::large_enum_variant)]
pub enum WorkerMessage {
    Ready {
        protocol_version: u32,
        max_frame_bytes: u64,
        pid: u32,
    },
    Accepted {
        id: u64,
    },
    Progress {
        id: u64,
        progress: ScanProgress,
    },
    Result {
        id: u64,
        report: ScanReport,
    },
    Error {
        id: Option<u64>,
        code: ErrorCode,
        message: String,
    },
    Goodbye,
}

#[cfg(test)]
mod tests {
    use super::*;
    use csvview_core::{InputInfo, ScanRequest};
    use std::io;

    #[test]
    fn client_messages_roundtrip() {
        let hello = ClientMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
        };
        let json = serde_json::to_string(&hello).unwrap();
        assert!(json.contains("\"type\":\"hello\""));
        assert_eq!(serde_json::from_str::<ClientMessage>(&json).unwrap(), hello);

        let scan = ClientMessage::Scan {
            id: 7,
            source: SourceSpec {
                path: "/tmp/x".into(),
                bytes: 12,
                truncated: false,
            },
            request: ScanRequest::full(),
        };
        let json = serde_json::to_string(&scan).unwrap();
        assert_eq!(serde_json::from_str::<ClientMessage>(&json).unwrap(), scan);
    }

    #[test]
    fn worker_messages_roundtrip_with_a_report() {
        let report = csvview_core::scan(
            io::Cursor::new(b"a,b\n1,2\n".to_vec()),
            InputInfo::known(8),
            &ScanRequest::full(),
        )
        .unwrap();
        let message = WorkerMessage::Result { id: 1, report };
        let json = serde_json::to_string(&message).unwrap();
        let decoded: WorkerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, message);
    }
}
