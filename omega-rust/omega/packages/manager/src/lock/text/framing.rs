use super::super::PackageLockError as Error;
use std::fmt::{self, Write};

pub(super) struct Writer {
    output: String,
    maximum: usize,
    error: Option<Error>,
}

impl Writer {
    pub(super) fn new(maximum: usize) -> Self {
        Self {
            output: String::new(),
            maximum,
            error: None,
        }
    }

    pub(super) fn append(&mut self, text: &str) -> Result<(), Error> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if self
            .output
            .len()
            .checked_add(text.len())
            .is_none_or(|count| count > self.maximum)
        {
            return Err(Error::ByteLimitExceeded);
        }
        self.output
            .try_reserve_exact(text.len())
            .map_err(|_| Error::AllocationFailed)?;
        self.output.push_str(text);
        Ok(())
    }

    pub(super) fn row(&mut self, label: &str, value: impl fmt::Display) -> Result<(), Error> {
        if writeln!(self, "{label} {value}").is_err() {
            return Err(self.error.clone().unwrap_or(Error::InvalidFraming));
        }
        Ok(())
    }

    pub(super) fn section(&mut self, name: &str, text: &str) -> Result<(), Error> {
        self.row(name, text.len())?;
        self.append(text)
    }

    pub(super) fn finish(self) -> Result<String, Error> {
        self.error.map_or(Ok(self.output), Err)
    }
}

impl Write for Writer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if let Err(error) = self.append(text) {
            self.error = Some(error);
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

pub(super) struct Reader<'text> {
    remaining: &'text str,
}

impl<'text> Reader<'text> {
    pub(super) fn new(remaining: &'text str) -> Self {
        Self { remaining }
    }

    pub(super) fn line(&mut self) -> Result<&'text str, Error> {
        let (line, remaining) = self
            .remaining
            .split_once('\n')
            .ok_or(Error::InvalidFraming)?;
        self.remaining = remaining;
        Ok(line)
    }

    pub(super) fn field(&mut self, label: &str) -> Result<&'text str, Error> {
        let line = self.line()?;
        let (found, value) = line.split_once(' ').ok_or(Error::InvalidFraming)?;
        if found == label && !value.is_empty() {
            Ok(value)
        } else {
            Err(Error::InvalidFraming)
        }
    }

    pub(super) fn count(&mut self, label: &str, maximum: usize) -> Result<usize, Error> {
        let number = self.field(label)?;
        if number.len() > 20
            || (number.len() > 1 && number.starts_with('0'))
            || !number.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(Error::InvalidFraming);
        }
        let number = number.parse::<usize>().map_err(|_| Error::InvalidFraming)?;
        if number > maximum {
            return Err(Error::CountLimitExceeded);
        }
        Ok(number)
    }

    pub(super) fn section(&mut self, label: &str, maximum: usize) -> Result<&'text str, Error> {
        let count = self.count(label, maximum)?;
        let (section, remaining) = self
            .remaining
            .split_at_checked(count)
            .ok_or(Error::InvalidFraming)?;
        self.remaining = remaining;
        Ok(section)
    }

    pub(super) fn expect(&mut self, expected: &str) -> Result<(), Error> {
        if self.line()? == expected {
            Ok(())
        } else {
            Err(Error::InvalidFraming)
        }
    }

    pub(super) fn finish(self) -> Result<(), Error> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(Error::InvalidFraming)
        }
    }
}
