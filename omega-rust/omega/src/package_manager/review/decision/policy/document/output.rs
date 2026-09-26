//! Bounded text construction shared by the review renderer.

use super::{PackagePolicyDecisionSubject, PackagePolicyReviewError};
use std::fmt::{self, Write};

pub(super) struct Output {
    pub text: String,
    pub subjects: Vec<PackagePolicyDecisionSubject>,
    maximum_bytes: usize,
    error: Option<PackagePolicyReviewError>,
}

impl Output {
    pub fn new(maximum_bytes: usize) -> Self {
        Self {
            text: String::new(),
            subjects: Vec::new(),
            maximum_bytes,
            error: None,
        }
    }

    pub fn finish(self, result: fmt::Result) -> Result<Self, PackagePolicyReviewError> {
        result.map_err(|_| {
            self.error
                .unwrap_or(PackagePolicyReviewError::AllocationFailed)
        })?;
        Ok(self)
    }

    pub fn choice(&mut self, subject: PackagePolicyDecisionSubject) -> fmt::Result {
        use PackagePolicyDecisionSubject as Subject;
        match subject {
            Subject::RootRole => self.write_str("decision root-role pending\n")?,
            Subject::SourceReplacement(digest) => {
                writeln!(self, "decision source-replacement {} pending", Hex(&digest))?
            }
            Subject::Row(digest) => writeln!(self, "decision row {} pending", Hex(&digest))?,
        }
        self.subjects.try_reserve(1).map_err(|_| {
            self.error = Some(PackagePolicyReviewError::AllocationFailed);
            fmt::Error
        })?;
        self.subjects.push(subject);
        Ok(())
    }
}

impl Write for Output {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.len() > self.maximum_bytes.saturating_sub(self.text.len()) {
            self.error = Some(PackagePolicyReviewError::ByteLimit);
            return Err(fmt::Error);
        }
        self.text.try_reserve(value.len()).map_err(|_| {
            self.error = Some(PackagePolicyReviewError::AllocationFailed);
            fmt::Error
        })?;
        self.text.push_str(value);
        Ok(())
    }
}

pub(super) struct Hex<'a>(pub &'a [u8]);
impl fmt::Display for Hex<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}
