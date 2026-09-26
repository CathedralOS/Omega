use std::fmt::{self, Write};

pub(super) struct Writer {
    output: String,
    maximum: usize,
}

impl Writer {
    pub(super) fn new(maximum: usize) -> Self {
        Self {
            output: String::new(),
            maximum,
        }
    }

    pub(super) fn append(&mut self, text: &str) -> Result<(), String> {
        if self
            .output
            .len()
            .checked_add(text.len())
            .is_none_or(|total| total > self.maximum)
        {
            return Err("package proposal exceeds text byte limit".into());
        }
        self.output
            .try_reserve_exact(text.len())
            .map_err(|_| "package proposal allocation failed".to_owned())?;
        self.output.push_str(text);
        Ok(())
    }

    pub(super) fn row(&mut self, label: &str, value: impl fmt::Display) -> Result<(), String> {
        // Only fixed labels, digests, catalog identities and bounded numbers
        // reach this temporary formatting allocation; payloads use append.
        self.append(&format!("{label} {value}\n"))
    }

    pub(super) fn section(&mut self, label: &str, text: &str) -> Result<(), String> {
        self.row(label, text.len())?;
        self.append(text)?;
        self.append("\n")
    }

    pub(super) fn finish(self) -> String {
        self.output
    }
}

pub(super) struct Reader<'text> {
    remaining: &'text str,
}

impl<'text> Reader<'text> {
    pub(super) fn new(remaining: &'text str) -> Self {
        Self { remaining }
    }

    fn line(&mut self) -> Result<&'text str, String> {
        let (line, remaining) = self
            .remaining
            .split_once('\n')
            .ok_or("invalid proposal framing")?;
        self.remaining = remaining;
        Ok(line)
    }

    pub(super) fn expect(&mut self, expected: &str) -> Result<(), String> {
        if self.line()? != expected {
            return Err("unexpected package proposal row".into());
        }
        Ok(())
    }

    pub(super) fn field(&mut self, label: &str) -> Result<&'text str, String> {
        let (found, value) = self
            .line()?
            .split_once(' ')
            .ok_or("invalid proposal field")?;
        if found != label || value.is_empty() {
            return Err("unexpected package proposal field".into());
        }
        Ok(value)
    }

    pub(super) fn count(&mut self, label: &str, maximum: usize) -> Result<usize, String> {
        let value = self.field(label)?;
        if value.len() > 20
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err("noncanonical package proposal count".into());
        }
        let count = value
            .parse::<usize>()
            .map_err(|_| "package proposal count overflow")?;
        if count > maximum {
            return Err("package proposal count exceeds limit".into());
        }
        Ok(count)
    }

    pub(super) fn section(&mut self, label: &str, maximum: usize) -> Result<&'text str, String> {
        let count = self.count(label, maximum)?;
        let (payload, remaining) = self
            .remaining
            .split_at_checked(count)
            .ok_or("invalid package proposal payload length or UTF-8 boundary")?;
        self.remaining = remaining
            .strip_prefix('\n')
            .ok_or("missing proposal framing LF")?;
        Ok(payload)
    }

    pub(super) fn finish(self) -> Result<(), String> {
        if !self.remaining.is_empty() {
            return Err("trailing package proposal bytes".into());
        }
        Ok(())
    }
}

pub(super) fn write_digest(digest: &[u8; 32]) -> String {
    let mut text = String::with_capacity(64);
    for byte in digest {
        write!(text, "{byte:02x}").expect("writing to a String cannot fail");
    }
    text
}

pub(super) fn read_digest(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("package proposal digest must be 64 lowercase hexadecimal digits".into());
    }
    let mut digest = [0; 32];
    for (destination, pair) in digest.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        let digit = |byte: u8| {
            if byte <= b'9' {
                byte - b'0'
            } else {
                byte - b'a' + 10
            }
        };
        *destination = digit(pair[0]) * 16 + digit(pair[1]);
    }
    Ok(digest)
}
