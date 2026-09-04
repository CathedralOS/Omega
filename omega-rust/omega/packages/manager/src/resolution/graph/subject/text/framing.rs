//! Bounded ASCII tokens and lossless quoted byte strings.

use super::Error;

pub(super) struct Writer {
    text: String,
    maximum: usize,
}

impl Writer {
    pub(super) fn new(maximum: usize) -> Self {
        Self {
            text: String::new(),
            maximum,
        }
    }

    fn append(&mut self, text: &str) -> Result<(), Error> {
        if self
            .text
            .len()
            .checked_add(text.len())
            .is_none_or(|length| length > self.maximum)
        {
            return Err(Error::new(
                "source-closure text exceeds its record-byte limit",
            ));
        }
        self.text
            .try_reserve(text.len())
            .map_err(|_| Error::new("source-closure text allocation failed"))?;
        self.text.push_str(text);
        Ok(())
    }

    pub(super) fn row(&mut self, label: &str, values: &[&[u8]]) -> Result<(), Error> {
        self.append(label)?;
        const HEX: &[u8] = b"0123456789abcdef";
        for value in values {
            self.append(" \"")?;
            for byte in *value {
                match byte {
                    b'"' => self.append("\\\"")?,
                    b'\\' => self.append("\\\\")?,
                    0x20..=0x7e => self.append(
                        std::str::from_utf8(std::slice::from_ref(byte)).expect("ASCII byte"),
                    )?,
                    _ => {
                        let escaped = [
                            b'\\',
                            b'x',
                            HEX[usize::from(byte >> 4)],
                            HEX[usize::from(byte & 15)],
                        ];
                        self.append(std::str::from_utf8(&escaped).expect("ASCII escape"))?;
                    }
                }
            }
            self.append("\"")?;
        }
        self.append("\n")
    }

    pub(super) fn number(&mut self, label: &str, value: usize) -> Result<(), Error> {
        self.row(&format!("{label} {value}"), &[])
    }

    pub(super) fn finish(self) -> String {
        self.text
    }
}

pub(super) struct Reader<'text> {
    text: &'text [u8],
    position: usize,
}

impl<'text> Reader<'text> {
    pub(super) fn new(text: &'text str, maximum: usize) -> Result<Self, Error> {
        if text.len() > maximum {
            return Err(Error::new(
                "source-closure text exceeds its record-byte limit",
            ));
        }
        Ok(Self {
            text: text.as_bytes(),
            position: 0,
        })
    }

    fn whitespace(&mut self) {
        while self
            .text
            .get(self.position)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.position += 1;
        }
    }

    pub(super) fn atom(&mut self) -> Result<&'text str, Error> {
        self.whitespace();
        let start = self.position;
        while self
            .text
            .get(self.position)
            .is_some_and(|byte| byte.is_ascii_graphic() && *byte != b'"')
        {
            self.position += 1;
            if self.position - start > 64 {
                return Err(Error::new("source-closure text token exceeds its limit"));
            }
        }
        if start == self.position {
            return Err(Error::new("expected source-closure text token"));
        }
        std::str::from_utf8(&self.text[start..self.position])
            .map_err(|_| Error::new("invalid source-closure text token"))
    }

    pub(super) fn expect(&mut self, expected: &str) -> Result<(), Error> {
        if self.atom()? == expected {
            Ok(())
        } else {
            Err(Error::new(
                "unexpected source-closure text field or version",
            ))
        }
    }

    pub(super) fn number(&mut self, maximum: usize) -> Result<usize, Error> {
        let token = self.atom()?;
        if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Error::new("invalid source-closure text count"));
        }
        let value = token
            .parse::<usize>()
            .map_err(|_| Error::new("source-closure text count overflows"))?;
        if value > maximum {
            return Err(Error::new("source-closure text count exceeds its limit"));
        }
        Ok(value)
    }

    pub(super) fn count(&mut self, maximum: usize) -> Result<usize, Error> {
        let count = self.number(maximum)?;
        // Every following record needs at least one byte. Reject impossible
        // large declarations before reserving their semantic collection.
        if count > self.text.len() - self.position {
            return Err(Error::new(
                "source-closure text count exceeds remaining framing",
            ));
        }
        Ok(count)
    }

    pub(super) fn bytes(&mut self, maximum: usize) -> Result<Vec<u8>, Error> {
        self.whitespace();
        if self.text.get(self.position) != Some(&b'"') {
            return Err(Error::new("expected quoted source-closure byte string"));
        }
        self.position += 1;
        let mut bytes = Vec::new();
        loop {
            let byte = self.take()?;
            let decoded = match byte {
                b'"' => return Ok(bytes),
                b'\\' => match self.take()? {
                    b'"' => b'"',
                    b'\\' => b'\\',
                    b'x' => {
                        let high = hex(self.take()?)?;
                        (high << 4) | hex(self.take()?)?
                    }
                    _ => return Err(Error::new("invalid source-closure byte escape")),
                },
                0x20..=0x7e => byte,
                _ => return Err(Error::new("unescaped source-closure byte")),
            };
            if bytes.len() == maximum {
                return Err(Error::new(
                    "source-closure text field exceeds its byte limit",
                ));
            }
            bytes
                .try_reserve(1)
                .map_err(|_| Error::new("source-closure field allocation failed"))?;
            bytes.push(decoded);
        }
    }

    fn take(&mut self) -> Result<u8, Error> {
        let byte = self
            .text
            .get(self.position)
            .copied()
            .ok_or_else(|| Error::new("truncated source-closure text"))?;
        self.position += 1;
        Ok(byte)
    }

    pub(super) fn string(&mut self, maximum: usize) -> Result<String, Error> {
        String::from_utf8(self.bytes(maximum)?)
            .map_err(|_| Error::new("source-closure identity or request requires UTF-8"))
    }

    pub(super) fn finish(&mut self) -> Result<(), Error> {
        self.whitespace();
        if self.position == self.text.len() {
            Ok(())
        } else {
            Err(Error::new("trailing source-closure text"))
        }
    }
}

fn hex(byte: u8) -> Result<u8, Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(Error::new("invalid lowercase hexadecimal byte escape")),
    }
}

pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::new("source-closure text collection allocation failed"))?;
    Ok(values)
}
