//! Bounded request/response frames for one binding's dedicated pair.
//!
//! One frame is one request or one response: `u32 operation`, `u32 payload
//! length`, then payload bytes — little-endian, at most `MAX_FRAME_BYTES`
//! total. An invalid length, truncation (premature EOF), or trailing bytes
//! is an explicit protocol failure: the mediator reports it and the
//! installer closes the binding. There is no framing for delegation,
//! endpoint payloads, or a second channel — authority-bearing endpoints
//! cannot appear in ordinary serialized payloads in this profile.
//!
//! Exact operation-schema validity lives with the per-contract codec a
//! binding's `transport` selects; this layer owns only the bounded envelope
//! every selected codec shares. The endpoint's `contract` identity already
//! binds which schema must check `operation` — equal byte shapes are not
//! compatibility evidence.

use std::fmt;

/// The largest encoded frame: header plus payload.
pub const MAX_FRAME_BYTES: usize = 4096;
const HEADER_BYTES: usize = 8;
const MAX_PAYLOAD_BYTES: usize = MAX_FRAME_BYTES - HEADER_BYTES;

/// One bounded request or response envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The operation identity within the endpoint's contract schema.
    pub operation: u32,
    pub payload: Vec<u8>,
}

/// A protocol failure on one binding's channel. Any of these closes the
/// binding: the peer forfeits further sends, and no reconnect to a
/// different peer is attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// The payload would push the encoded frame past the bound.
    Oversized { payload: usize },
    /// Fewer bytes than the header or declared payload require.
    Truncated { expected: usize, found: usize },
    /// Extra bytes follow one complete frame.
    TrailingBytes { count: usize },
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { payload } => write!(
                formatter,
                "payload of {payload} bytes exceeds the bounded frame"
            ),
            Self::Truncated { expected, found } => write!(
                formatter,
                "frame truncated: expected {expected} bytes, found {found}"
            ),
            Self::TrailingBytes { count } => {
                write!(formatter, "{count} trailing bytes after the frame")
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// Serialize one frame. Fails rather than emitting an oversized envelope.
pub fn encode_frame(frame: &Frame) -> Result<Vec<u8>, FrameError> {
    if frame.payload.len() > MAX_PAYLOAD_BYTES {
        return Err(FrameError::Oversized {
            payload: frame.payload.len(),
        });
    }
    let mut bytes = Vec::with_capacity(HEADER_BYTES + frame.payload.len());
    bytes.extend_from_slice(&frame.operation.to_le_bytes());
    bytes.extend_from_slice(&(frame.payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&frame.payload);
    Ok(bytes)
}

/// Reconstruct exactly one frame from a complete channel read. The input
/// must be the whole bounded envelope: a short read, a declared length that
/// overruns the bytes, or anything after the payload is a protocol failure.
pub fn decode_frame(bytes: &[u8]) -> Result<Frame, FrameError> {
    if bytes.len() < HEADER_BYTES {
        return Err(FrameError::Truncated {
            expected: HEADER_BYTES,
            found: bytes.len(),
        });
    }
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TrailingBytes {
            count: bytes.len() - MAX_FRAME_BYTES,
        });
    }
    let operation = u32::from_le_bytes(bytes[0..4].try_into().expect("header slice"));
    let length = u32::from_le_bytes(bytes[4..8].try_into().expect("header slice")) as usize;
    let expected = HEADER_BYTES + length;
    if bytes.len() < expected {
        return Err(FrameError::Truncated {
            expected,
            found: bytes.len(),
        });
    }
    if bytes.len() > expected {
        return Err(FrameError::TrailingBytes {
            count: bytes.len() - expected,
        });
    }
    Ok(Frame {
        operation,
        payload: bytes[HEADER_BYTES..].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_round_trips() {
        let frame = Frame {
            operation: 7,
            payload: vec![1, 2, 3],
        };
        let bytes = encode_frame(&frame).unwrap();
        assert_eq!(decode_frame(&bytes), Ok(frame));
    }

    #[test]
    fn the_bound_rejects_on_both_sides() {
        let oversized = Frame {
            operation: 0,
            payload: vec![0; MAX_PAYLOAD_BYTES + 1],
        };
        assert!(matches!(
            encode_frame(&oversized),
            Err(FrameError::Oversized { .. })
        ));
        let maximum = Frame {
            operation: 0,
            payload: vec![0; MAX_PAYLOAD_BYTES],
        };
        let bytes = encode_frame(&maximum).unwrap();
        assert_eq!(decode_frame(&bytes), Ok(maximum));
    }

    #[test]
    fn truncation_and_trailing_bytes_reject() {
        let frame = Frame {
            operation: 1,
            payload: vec![9; 4],
        };
        let bytes = encode_frame(&frame).unwrap();
        assert!(matches!(
            decode_frame(&bytes[..bytes.len() - 1]),
            Err(FrameError::Truncated { .. })
        ));
        assert!(matches!(
            decode_frame(&bytes[..4]),
            Err(FrameError::Truncated { .. })
        ));
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            decode_frame(&trailing),
            Err(FrameError::TrailingBytes { .. })
        ));
    }
}
