//! Bounded ASCII tokens and lossless quoted byte strings.

use super::super::usage::Budget;
use super::Error;

pub(super) struct Writer<'expected> {
    text: String,
    maximum: usize,
    expected: Option<&'expected str>,
    position: usize,
    reserved: usize,
    pub(super) budget: Budget,
}

impl<'expected> Writer<'expected> {
    pub(super) fn new(maximum: usize, budget: Budget) -> Self {
        Self {
            text: String::new(),
            maximum,
            expected: None,
            position: 0,
            reserved: 0,
            budget,
        }
    }

    pub(super) fn verifying(maximum: usize, expected: &'expected str, budget: Budget) -> Self {
        Self {
            expected: Some(expected),
            ..Self::new(maximum, budget)
        }
    }

    fn append(&mut self, text: &str) -> Result<(), Error> {
        let end = self
            .position
            .checked_add(text.len())
            .filter(|length| *length <= self.maximum)
            .ok_or_else(|| Error::new("source-closure text exceeds its record-byte limit"))?;
        if let Some(expected) = self.expected {
            if expected.as_bytes().get(self.position..end) != Some(text.as_bytes()) {
                return Err(Error::new("source-closure text is not canonical"));
            }
        } else {
            if end > self.reserved {
                let capacity = self.reserved.saturating_mul(2).max(end).min(self.maximum);
                self.budget.charge(capacity - self.reserved)?;
                self.text
                    .try_reserve_exact(capacity - self.text.len())
                    .map_err(|_| Error::new("source-closure text allocation failed"))?;
                self.reserved = capacity;
            }
            self.text.push_str(text);
        }
        self.position = end;
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
        self.append(label)?;
        self.append(" ")?;
        let mut digits = [0u8; 20];
        let mut cursor = digits.len();
        let mut remainder = value;
        loop {
            cursor -= 1;
            digits[cursor] = b'0' + u8::try_from(remainder % 10).expect("decimal digit");
            remainder /= 10;
            if remainder == 0 {
                break;
            }
        }
        self.append(std::str::from_utf8(&digits[cursor..]).expect("ASCII decimal"))?;
        self.append("\n")
    }

    pub(super) fn finish(self) -> Result<(String, Budget), Error> {
        if self
            .expected
            .is_some_and(|expected| expected.len() != self.position)
        {
            return Err(Error::new("source-closure text is not canonical"));
        }
        Ok((self.text, self.budget))
    }
}

pub(super) struct Reader<'text> {
    text: &'text [u8],
    position: usize,
    pub(super) budget: Budget,
}

impl<'text> Reader<'text> {
    pub(super) fn new(text: &'text str, maximum: usize, budget: Budget) -> Result<Self, Error> {
        if text.len() > maximum {
            return Err(Error::new(
                "source-closure text exceeds its record-byte limit",
            ));
        }
        Ok(Self {
            text: text.as_bytes(),
            position: 0,
            budget,
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
        let start = self.position;
        let mut length = 0usize;
        while self.decoded_byte()?.is_some() {
            if length == maximum {
                return Err(Error::new(
                    "source-closure text field exceeds its byte limit",
                ));
            }
            length += 1;
        }
        let mut bytes = self.budget.reserve(length)?;
        self.position = start;
        while let Some(byte) = self.decoded_byte()? {
            bytes.push(byte);
        }
        Ok(bytes)
    }

    fn decoded_byte(&mut self) -> Result<Option<u8>, Error> {
        let byte = self.take()?;
        let decoded = match byte {
            b'"' => return Ok(None),
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
        Ok(Some(decoded))
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
