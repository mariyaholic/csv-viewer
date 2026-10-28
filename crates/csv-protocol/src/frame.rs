// SPDX-License-Identifier: Apache-2.0

//! Length-prefixed, bounded message framing.
//!
//! Every frame is a 4-byte big-endian length followed by JSON. A frame larger
//! than the configured maximum is rejected before allocation, so a broken or
//! hostile peer cannot request unbounded memory.

use std::fmt;
use std::io::{self, Read, Write};

use serde::de::DeserializeOwned;
use serde::Serialize;

pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub enum FrameError {
    Closed,
    Truncated,
    Empty,
    TooLarge { length: usize, maximum: usize },
    Io(io::Error),
    Json(String),
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::Closed => write!(formatter, "connection closed"),
            FrameError::Truncated => write!(formatter, "truncated frame"),
            FrameError::Empty => write!(formatter, "empty frame"),
            FrameError::TooLarge { length, maximum } => {
                write!(formatter, "frame of {length} bytes exceeds maximum {maximum}")
            }
            FrameError::Io(error) => write!(formatter, "I/O error: {error}"),
            FrameError::Json(message) => write!(formatter, "invalid message: {message}"),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<io::Error> for FrameError {
    fn from(error: io::Error) -> Self {
        FrameError::Io(error)
    }
}

/// Read one message. `Closed` is returned only when no byte was read; a
/// partial length or payload is `Truncated`.
pub fn read_message<R: Read, T: DeserializeOwned>(
    reader: &mut R,
    maximum: usize,
) -> Result<T, FrameError> {
    let mut prefix = [0u8; 4];
    match read_exact_or_closed(reader, &mut prefix)? {
        ReadState::Closed => return Err(FrameError::Closed),
        ReadState::Complete => {}
    }
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 {
        return Err(FrameError::Empty);
    }
    if length > maximum {
        return Err(FrameError::TooLarge { length, maximum });
    }
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            FrameError::Truncated
        } else {
            FrameError::Io(error)
        }
    })?;
    serde_json::from_slice(&payload).map_err(|error| FrameError::Json(error.to_string()))
}

enum ReadState {
    Closed,
    Complete,
}

fn read_exact_or_closed<R: Read>(reader: &mut R, buffer: &mut [u8]) -> Result<ReadState, FrameError> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) if filled == 0 => return Ok(ReadState::Closed),
            Ok(0) => return Err(FrameError::Truncated),
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(FrameError::Io(error)),
        }
    }
    Ok(ReadState::Complete)
}

pub fn write_message<W: Write, T: Serialize>(
    writer: &mut W,
    message: &T,
    maximum: usize,
) -> Result<usize, FrameError> {
    let payload =
        serde_json::to_vec(message).map_err(|error| FrameError::Json(error.to_string()))?;
    if payload.len() > maximum {
        return Err(FrameError::TooLarge {
            length: payload.len(),
            maximum,
        });
    }
    if payload.is_empty() {
        return Err(FrameError::Empty);
    }
    let length = u32::try_from(payload.len()).map_err(|_| FrameError::TooLarge {
        length: payload.len(),
        maximum,
    })?;
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(payload.len() + 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Ping {
        value: String,
    }

    #[test]
    fn roundtrips_a_message() {
        let mut buffer = Vec::new();
        let written = write_message(
            &mut buffer,
            &Ping {
                value: "hello".into(),
            },
            MAX_FRAME_BYTES,
        )
        .unwrap();
        assert_eq!(written, buffer.len());
        let mut cursor = std::io::Cursor::new(buffer);
        let message: Ping = read_message(&mut cursor, MAX_FRAME_BYTES).unwrap();
        assert_eq!(
            message,
            Ping {
                value: "hello".into()
            }
        );
    }

    #[test]
    fn rejects_oversized_before_allocating() {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&(100u32).to_be_bytes());
        let error = read_message::<_, Ping>(&mut std::io::Cursor::new(buffer), 10).unwrap_err();
        assert!(matches!(
            error,
            FrameError::TooLarge {
                length: 100,
                maximum: 10
            }
        ));
    }

    #[test]
    fn distinguishes_closed_from_truncated() {
        let error = read_message::<_, Ping>(&mut std::io::Cursor::new(Vec::new()), 10).unwrap_err();
        assert!(matches!(error, FrameError::Closed));
        let error = read_message::<_, Ping>(&mut std::io::Cursor::new(vec![0, 0]), 10).unwrap_err();
        assert!(matches!(error, FrameError::Truncated));
        let mut payload = vec![0, 0, 0, 4, b'{'];
        payload.extend_from_slice(b"}  ");
        let error = read_message::<_, Ping>(&mut std::io::Cursor::new(payload), 10).unwrap_err();
        assert!(matches!(error, FrameError::Json(_)));
    }

    #[test]
    fn rejects_empty_and_invalid_json() {
        let error =
            read_message::<_, Ping>(&mut std::io::Cursor::new(vec![0, 0, 0, 0]), 10).unwrap_err();
        assert!(matches!(error, FrameError::Empty));

        let mut buffer = Vec::new();
        buffer.extend_from_slice(&4u32.to_be_bytes());
        buffer.extend_from_slice(b"nope");
        let error = read_message::<_, Ping>(&mut std::io::Cursor::new(buffer), 10).unwrap_err();
        assert!(matches!(error, FrameError::Json(_)));
    }
}
