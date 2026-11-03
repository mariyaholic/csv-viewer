// SPDX-License-Identifier: Apache-2.0
//! Desktop-only, bounded request/reply protocol; Explorer protocols stay unchanged.
use crate::Table;
use serde::{Deserialize, Serialize};
use std::{
    fs::Metadata,
    io::{Read, Write},
};
pub const MAX_FRAME: usize = 16 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    pub bytes: u64,
    pub modified_ns: Option<u64>,
}
impl Stamp {
    pub fn from_metadata(m: &Metadata) -> Self {
        Self {
            bytes: m.len(),
            modified_ns: m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos().min(u64::MAX as u128) as u64),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub offset: u64,
    pub query: String,
    pub expected: Option<Stamp>,
    pub byte_offset: Option<u64>,
}
impl Request {
    pub fn validate(&self) -> Result<(), String> {
        if self.byte_offset.is_some() && self.expected.is_none() {
            return Err("CSV checkpoints require unchanged-source metadata".into());
        }
        if self.query.len() > 1024 {
            Err("Search is limited to 1024 UTF-8 bytes".into())
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Page {
    pub start: u64,
    pub next: Option<u64>,
    pub source_rows: Vec<u64>,
    pub stamp: Stamp,
    pub next_byte: Option<u64>,
    pub scanned_to: u64,
}
#[derive(Default)]
pub struct Navigation {
    pub current: Request,
    history: Vec<Request>,
}
impl Navigation {
    pub fn previous(&self) -> Option<Request> {
        self.history.last().cloned()
    }
    pub fn history_len(&self) -> usize {
        self.history.len()
    }
    pub fn commit(&mut self, request: Request) {
        if request.offset == 0 || request.query != self.current.query {
            self.history.clear();
        } else if self
            .history
            .last()
            .is_some_and(|p| p.offset == request.offset)
        {
            self.history.pop();
        } else if request.offset != self.current.offset {
            if self.history.len() == 256 {
                self.history.remove(0);
            }
            self.history.push(self.current.clone());
        }
        self.current = request;
    }
}
pub fn write<T: Serialize>(out: &mut dyn Write, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FRAME {
        return Err("Desktop response exceeds 16 MiB".into());
    }
    out.write_all(&(bytes.len() as u32).to_le_bytes())
        .and_then(|_| out.write_all(&bytes))
        .and_then(|_| out.flush())
        .map_err(|e| e.to_string())
}
pub fn read<T: serde::de::DeserializeOwned>(input: &mut dyn Read) -> Result<T, String> {
    let mut size = [0; 4];
    input
        .read_exact(&mut size)
        .map_err(|e| format!("Desktop worker closed: {e}"))?;
    let n = u32::from_le_bytes(size) as usize;
    if n > MAX_FRAME {
        return Err("Desktop frame exceeds 16 MiB".into());
    }
    let mut bytes = vec![0; n];
    input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
/// Worker mode is explicit; it never creates a window or changes associations.
pub fn serve(
    path: &std::path::Path,
    handler: fn(&std::path::Path, &Request) -> Result<Table, String>,
) -> Result<(), String> {
    let request: Request = read(&mut std::io::stdin().lock())?;
    request.validate()?;
    let result = handler(path, &request).map(Table::bounded);
    let mut out = std::io::stdout().lock();
    if let Err(e) = write(&mut out, &result) {
        write(&mut out, &Result::<Table, String>::Err(e))?;
    }
    Ok(())
}
