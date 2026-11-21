//! Read the records that one model audit observes.
use serde_json::Value;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Take};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Format {
    Antigravity,
    Claude,
    Codex,
}

impl Format {
    fn from_record(record: &Value) -> Option<Self> {
        match record["type"].as_str() {
            Some("PLANNER_RESPONSE" | "USER_INPUT") => Some(Self::Antigravity),
            Some("api-request") => Some(Self::Claude),
            Some("turn_context") => Some(Self::Codex),
            _ if record.get("parentUuid").is_some() => Some(Self::Claude),
            _ => None,
        }
    }

    pub(super) fn detect(path: &Path) -> Option<Self> {
        let mut transcript = Transcript::open(path).ok()?;
        while transcript.next_record().ok()?.is_some() {
            if let Some(format) = transcript.format {
                return Some(format);
            }
        }
        transcript.format()
    }
}

pub(super) struct Transcript {
    reader: BufReader<Take<File>>,
    line: String,
    format: Option<Format>,
    session_header: bool,
}

impl Transcript {
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        // A running agent can append while its report is being prepared.
        let metadata = file.metadata()?;
        let length = if metadata.is_file() {
            metadata.len()
        } else {
            u64::MAX
        };
        Ok(Self {
            reader: BufReader::new(file.take(length)),
            line: String::new(),
            format: None,
            session_header: false,
        })
    }

    pub(super) fn format(&self) -> Option<Format> {
        self.format.or(self.session_header.then_some(Format::Codex))
    }

    pub(super) fn next_record(&mut self) -> io::Result<Option<Value>> {
        loop {
            self.line.clear();
            if self.reader.read_line(&mut self.line)? == 0 {
                return Ok(None);
            }
            if let Ok(record) = serde_json::from_str::<Value>(&self.line) {
                if self.format.is_none() {
                    // An exported transcript can have a session header before
                    // its first agent-specific record.
                    if record["type"].as_str() == Some("session_meta") {
                        self.session_header = true;
                    }
                    self.format = Format::from_record(&record);
                }
                return Ok(Some(record));
            }
        }
    }
}
