use super::{HEADER, MAXIMUM_MARKUP_DEPTH, label};
use crate::encoding::PackageReviewEncodingError as Error;
use std::fmt::{self, Write};

#[derive(Clone, Copy, Default)]
struct Scope {
    permits_scalar: bool,
    has_scalar: bool,
}

/// Verification borrows the original text and compares each emitted byte. It
/// does not allocate a second expanded text buffer alongside recovered policy.
pub(in crate::encoding) struct Writer<'text> {
    output: String,
    expected: Option<&'text [u8]>,
    position: usize,
    maximum: usize,
    scopes: [Scope; MAXIMUM_MARKUP_DEPTH],
    depth: usize,
    error: Option<&'static str>,
}

impl<'text> Writer<'text> {
    pub(in crate::encoding) fn new(maximum: usize, expected: Option<&'text str>) -> Self {
        let mut writer = Self {
            output: String::new(),
            expected: expected.map(str::as_bytes),
            position: 0,
            maximum,
            scopes: [Scope::default(); MAXIMUM_MARKUP_DEPTH],
            depth: 0,
            error: None,
        };
        writer.append(HEADER);
        writer
    }

    pub(in crate::encoding) fn fail(&mut self, message: &'static str) {
        self.error.get_or_insert(message);
    }

    pub(in crate::encoding) fn check(&self) -> Result<(), Error> {
        self.error
            .map_or(Ok(()), |message| Err(Error::new(message)))
    }

    pub(in crate::encoding) fn finish(self) -> Result<String, Error> {
        self.check()?;
        if self.depth != 0
            || self
                .expected
                .is_some_and(|expected| self.position != expected.len())
        {
            return Err(Error::new(
                "package policy text has noncanonical trailing structure",
            ));
        }
        Ok(self.output)
    }

    fn append(&mut self, value: &str) {
        if self.error.is_some() {
            return;
        }
        let Some(end) = self
            .position
            .checked_add(value.len())
            .filter(|end| *end <= self.maximum)
        else {
            self.fail("package policy text exceeds its byte ceiling");
            return;
        };
        if let Some(expected) = self.expected {
            if expected.get(self.position..end) != Some(value.as_bytes()) {
                self.fail("package policy text is not canonical");
                return;
            }
        } else {
            if self.output.try_reserve(value.len()).is_err() {
                self.fail("package policy text allocation failed");
                return;
            }
            self.output.push_str(value);
        }
        self.position = end;
    }

    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.append("  ");
        }
    }

    fn open(&mut self, permits_scalar: bool) {
        self.append(" {\n");
        if self.depth == MAXIMUM_MARKUP_DEPTH {
            self.fail("package policy text exceeds its markup nesting ceiling");
            return;
        }
        self.scopes[self.depth] = Scope {
            permits_scalar,
            has_scalar: false,
        };
        self.depth += 1;
    }

    pub(in crate::encoding) fn end(&mut self) {
        if self.depth == 0 {
            self.fail("package policy text scope underflow");
            return;
        }
        self.depth -= 1;
        self.indent();
        self.append("}\n");
    }

    pub(in crate::encoding) fn scope(&mut self, kind: &'static str, name: &'static str) {
        if !label(name) {
            self.fail("package policy text has an invalid authored field label");
        }
        self.indent();
        self.append(kind);
        self.append(" ");
        self.append(name);
        self.open(kind == "field");
    }

    fn scalar(&mut self) {
        let Some(scope) = self
            .depth
            .checked_sub(1)
            .and_then(|index| self.scopes.get_mut(index))
        else {
            self.fail("package policy text scalar has no named field");
            return;
        };
        if !scope.permits_scalar || scope.has_scalar {
            self.fail("package policy text compound value needs named fields");
            return;
        }
        scope.has_scalar = true;
        self.indent();
    }

    pub(in crate::encoding) fn number(&mut self, kind: &'static str, value: impl fmt::Display) {
        self.scalar();
        self.append(kind);
        self.append(" ");
        let _ = writeln!(self, "{value}");
    }

    pub(in crate::encoding) fn boolean(&mut self, value: bool) {
        self.scalar();
        self.append(if value { "bool true\n" } else { "bool false\n" });
    }

    pub(in crate::encoding) fn tag(&mut self, name: &'static str, value: u8) {
        if !label(name) {
            self.fail("package policy text has an invalid authored variant label");
        }
        self.scalar();
        self.append("tag ");
        self.append(name);
        let _ = writeln!(self, " {value}");
    }

    pub(in crate::encoding) fn sequence(&mut self, count: u64) {
        self.scalar();
        let _ = write!(self, "sequence {count}");
        self.open(false);
    }

    pub(in crate::encoding) fn item(&mut self) {
        self.indent();
        self.append("item");
        self.open(true);
    }

    pub(in crate::encoding) fn option(&mut self, present: bool) {
        self.scalar();
        if present {
            self.append("option some");
            self.open(true);
        } else {
            self.append("option none\n");
        }
    }

    pub(in crate::encoding) fn bytes(&mut self, kind: &'static str, value: &[u8]) {
        self.scalar();
        if kind == "fixed" && value.len() == 32 {
            self.append("digest ");
            for byte in value {
                let _ = write!(self, "{byte:02x}");
            }
            self.append("\n");
            return;
        }
        self.append(kind);
        self.append(" \"");
        for byte in value {
            match byte {
                b'"' => self.append("\\\""),
                b'\\' => self.append("\\\\"),
                0x20..=0x7e => {
                    self.append(
                        std::str::from_utf8(std::slice::from_ref(byte)).expect("ASCII byte"),
                    );
                }
                _ => {
                    let _ = write!(self, "\\x{byte:02x}");
                }
            }
        }
        self.append("\"\n");
    }
}

impl Write for Writer<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.append(value);
        if self.error.is_some() {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}
