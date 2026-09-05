use crate::encoding::PackagePolicyRecoveryError as Error;

pub(super) struct Tokens<'text> {
    remaining: &'text [u8],
}

impl<'text> Tokens<'text> {
    pub(super) fn new(text: &'text str) -> Self {
        Self {
            remaining: text.as_bytes(),
        }
    }

    fn whitespace(&mut self) {
        let count = self
            .remaining
            .iter()
            .take_while(|byte| byte.is_ascii_whitespace())
            .count();
        self.remaining = &self.remaining[count..];
    }

    pub(super) fn atom(&mut self) -> Result<&'text str, Error> {
        self.whitespace();
        let count = self
            .remaining
            .iter()
            .take_while(|byte| byte.is_ascii_graphic() && **byte != b'"')
            .count();
        if count == 0 || count > 64 {
            return Err(Error::InvalidValue);
        }
        let (token, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        std::str::from_utf8(token).map_err(|_| Error::InvalidUtf8)
    }

    pub(super) fn expect(&mut self, expected: &str) -> Result<(), Error> {
        if self.atom()? == expected {
            Ok(())
        } else {
            Err(Error::InvalidValue)
        }
    }

    pub(super) fn done(&mut self) -> bool {
        self.whitespace();
        self.remaining.is_empty()
    }

    /// Borrow the encoded field. Counting and decoding can each traverse it
    /// without allocating a temporary unescaped string.
    pub(super) fn quoted(&mut self) -> Result<&'text [u8], Error> {
        self.whitespace();
        if self.remaining.first() != Some(&b'"') {
            return Err(Error::InvalidValue);
        }
        let source = &self.remaining[1..];
        let mut index = 0;
        loop {
            match source.get(index).copied().ok_or(Error::UnexpectedEnd)? {
                b'"' => {
                    self.remaining = &source[index + 1..];
                    return Ok(&source[..index]);
                }
                b'\\' => {
                    index += 1;
                    match source.get(index).copied().ok_or(Error::UnexpectedEnd)? {
                        b'"' | b'\\' => index += 1,
                        b'x' => {
                            nibble(*source.get(index + 1).ok_or(Error::UnexpectedEnd)?)?;
                            nibble(*source.get(index + 2).ok_or(Error::UnexpectedEnd)?)?;
                            index += 3;
                        }
                        _ => return Err(Error::InvalidValue),
                    }
                }
                0x20..=0x7e => index += 1,
                _ => return Err(Error::InvalidValue),
            }
        }
    }
}

pub(super) fn decoded(value: &[u8]) -> impl Iterator<Item = u8> + '_ {
    let mut index = 0;
    std::iter::from_fn(move || {
        let byte = *value.get(index)?;
        index += 1;
        if byte != b'\\' {
            return Some(byte);
        }
        let escaped = value[index];
        index += 1;
        if escaped != b'x' {
            return Some(escaped);
        }
        let byte = nibble(value[index]).expect("validated escape") * 16
            + nibble(value[index + 1]).expect("validated escape");
        index += 2;
        Some(byte)
    })
}

pub(super) fn nibble(byte: u8) -> Result<u8, Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(Error::InvalidValue),
    }
}
