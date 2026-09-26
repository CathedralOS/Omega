//! Byte-bounded escaped output sink.

use super::PackageSourcePatchError;
use super::diff::SourceLine;

pub(super) fn render_source_line(
    output: &mut BoundedOutput,
    lane: &str,
    line: SourceLine<'_>,
) -> Result<(), PackageSourcePatchError> {
    output.push(lane)?;
    output.push(if line.has_lf { " lf " } else { " none " })?;
    output.push_escaped(line.bytes)?;
    output.push("\n")
}

pub(super) struct BoundedOutput {
    maximum_bytes: usize,
    rendered: String,
}

impl BoundedOutput {
    pub(super) fn new(maximum_bytes: usize) -> Self {
        Self {
            maximum_bytes,
            rendered: String::new(),
        }
    }

    pub(super) fn push(&mut self, value: &str) -> Result<(), PackageSourcePatchError> {
        let required_at_least = self.rendered.len().saturating_add(value.len());
        if required_at_least > self.maximum_bytes {
            return Err(PackageSourcePatchError::OutputExceeded {
                maximum_bytes: self.maximum_bytes,
                required_at_least,
            });
        }
        self.rendered.push_str(value);
        Ok(())
    }

    pub(super) fn push_usize(&mut self, value: usize) -> Result<(), PackageSourcePatchError> {
        self.push(&value.to_string())
    }

    pub(super) fn push_hex(&mut self, bytes: &[u8]) -> Result<(), PackageSourcePatchError> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        for byte in bytes {
            let encoded = [
                DIGITS[usize::from(byte >> 4)] as char,
                DIGITS[usize::from(byte & 0x0f)] as char,
            ];
            for digit in encoded {
                let mut buffer = [0_u8; 4];
                self.push(digit.encode_utf8(&mut buffer))?;
            }
        }
        Ok(())
    }

    pub(super) fn push_escaped(&mut self, bytes: &[u8]) -> Result<(), PackageSourcePatchError> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        for byte in bytes {
            match *byte {
                b'\\' => self.push("\\\\")?,
                0x20..=0x7e => {
                    let literal = [*byte];
                    self.push(std::str::from_utf8(&literal).expect("printable ASCII is UTF-8"))?;
                }
                byte => {
                    let escaped = [
                        b'\\',
                        b'x',
                        DIGITS[usize::from(byte >> 4)],
                        DIGITS[usize::from(byte & 0x0f)],
                    ];
                    self.push(std::str::from_utf8(&escaped).expect("hex escape is ASCII"))?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn finish(self) -> String {
        self.rendered
    }
}
