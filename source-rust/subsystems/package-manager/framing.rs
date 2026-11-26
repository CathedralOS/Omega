// Length-delimited ASCII rows and sections shared by `omega.lock` and pending
// package proposals. Each format keeps its own refusal for every failure point
// (`FramingErrors`) and chooses whether a section payload ends with an LF, so
// one reader and writer serve both without changing either file's bytes or
// messages.

use std::fmt::{self, Write};

/// One refusal per failure point, in the error type of the owning format.
pub(crate) struct FramingErrors<E: 'static> {
    pub(crate) byte_limit: E,
    pub(crate) allocation: E,
    pub(crate) missing_line: E,
    pub(crate) unexpected_line: E,
    pub(crate) malformed_field: E,
    pub(crate) unexpected_field: E,
    pub(crate) noncanonical_count: E,
    pub(crate) count_overflow: E,
    pub(crate) count_limit: E,
    pub(crate) section_length: E,
    pub(crate) missing_section_terminator: E,
    pub(crate) trailing_bytes: E,
}

/// A framed text format: its refusals and whether each section payload is
/// followed by an LF.
pub(crate) struct Framing<E: 'static> {
    pub(crate) errors: FramingErrors<E>,
    pub(crate) section_terminator: bool,
}

pub(crate) struct Writer<E: Clone + 'static> {
    output: String,
    maximum: usize,
    framing: &'static Framing<E>,
    error: Option<E>,
}

impl<E: Clone + 'static> Writer<E> {
    pub(crate) fn new(framing: &'static Framing<E>, maximum: usize) -> Self {
        Self {
            output: String::new(),
            maximum,
            framing,
            error: None,
        }
    }

    pub(crate) fn append(&mut self, text: &str) -> Result<(), E> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let errors = &self.framing.errors;
        if self
            .output
            .len()
            .checked_add(text.len())
            .is_none_or(|count| count > self.maximum)
        {
            return Err(errors.byte_limit.clone());
        }
        self.output
            .try_reserve_exact(text.len())
            .map_err(|_| errors.allocation.clone())?;
        self.output.push_str(text);
        Ok(())
    }

    pub(crate) fn row(&mut self, label: &str, value: impl fmt::Display) -> Result<(), E> {
        if writeln!(self, "{label} {value}").is_err() {
            return Err(self
                .error
                .clone()
                .unwrap_or_else(|| self.framing.errors.missing_line.clone()));
        }
        Ok(())
    }

    pub(crate) fn section(&mut self, label: &str, text: &str) -> Result<(), E> {
        self.row(label, text.len())?;
        self.append(text)?;
        if self.framing.section_terminator {
            self.append("\n")?;
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<String, E> {
        self.error.map_or(Ok(self.output), Err)
    }
}

impl<E: Clone + 'static> Write for Writer<E> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if let Err(error) = self.append(text) {
            self.error = Some(error);
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

pub(crate) struct Reader<'text, E: Clone + 'static> {
    remaining: &'text str,
    framing: &'static Framing<E>,
}

impl<'text, E: Clone + 'static> Reader<'text, E> {
    pub(crate) fn new(framing: &'static Framing<E>, remaining: &'text str) -> Self {
        Self { remaining, framing }
    }

    fn errors(&self) -> &'static FramingErrors<E> {
        &self.framing.errors
    }

    pub(crate) fn line(&mut self) -> Result<&'text str, E> {
        let (line, remaining) = self
            .remaining
            .split_once('\n')
            .ok_or_else(|| self.errors().missing_line.clone())?;
        self.remaining = remaining;
        Ok(line)
    }

    pub(crate) fn expect(&mut self, expected: &str) -> Result<(), E> {
        if self.line()? == expected {
            Ok(())
        } else {
            Err(self.errors().unexpected_line.clone())
        }
    }

    pub(crate) fn field(&mut self, label: &str) -> Result<&'text str, E> {
        let line = self.line()?;
        let (found, value) = line
            .split_once(' ')
            .ok_or_else(|| self.errors().malformed_field.clone())?;
        if found == label && !value.is_empty() {
            Ok(value)
        } else {
            Err(self.errors().unexpected_field.clone())
        }
    }

    pub(crate) fn count(&mut self, label: &str, maximum: usize) -> Result<usize, E> {
        let number = self.field(label)?;
        let errors = self.errors();
        if number.len() > 20
            || (number.len() > 1 && number.starts_with('0'))
            || !number.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(errors.noncanonical_count.clone());
        }
        let number = number
            .parse::<usize>()
            .map_err(|_| errors.count_overflow.clone())?;
        if number > maximum {
            return Err(errors.count_limit.clone());
        }
        Ok(number)
    }

    pub(crate) fn section(&mut self, label: &str, maximum: usize) -> Result<&'text str, E> {
        let count = self.count(label, maximum)?;
        let errors = self.errors();
        let (section, mut remaining) = self
            .remaining
            .split_at_checked(count)
            .ok_or_else(|| errors.section_length.clone())?;
        if self.framing.section_terminator {
            remaining = remaining
                .strip_prefix('\n')
                .ok_or_else(|| errors.missing_section_terminator.clone())?;
        }
        self.remaining = remaining;
        Ok(section)
    }

    pub(crate) fn finish(self) -> Result<(), E> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(self.errors().trailing_bytes.clone())
        }
    }
}
