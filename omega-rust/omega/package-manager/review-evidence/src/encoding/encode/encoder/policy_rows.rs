//! Allocation-free sizing and exact-reservation dual row output.
use super::{Encoder, PackageReviewEncodingError, text::Writer};

impl Encoder<'_> {
    pub(in crate::encoding) fn row_measure(
        maximum_bytes: usize,
        maximum_text: Option<usize>,
        elements: usize,
        depth: usize,
    ) -> Self {
        Self {
            measure_only: true,
            policy_elements: Some(elements),
            maximum_policy_depth: depth,
            text: maximum_text.map(Writer::measured),
            ..Self::bounded(maximum_bytes)
        }
    }

    pub(in crate::encoding) fn row_output(
        bytes: usize,
        text: Option<usize>,
        elements: usize,
        depth: usize,
    ) -> Result<Self, PackageReviewEncodingError> {
        let mut encoder = Self {
            policy_elements: Some(elements),
            maximum_policy_depth: depth,
            retain_binary_with_text: text.is_some(),
            text: text.map(Writer::preallocated).transpose()?,
            ..Self::bounded(bytes)
        };
        encoder
            .output
            .try_reserve_exact(bytes)
            .map_err(|_| PackageReviewEncodingError::new("package policy row allocation failed"))?;
        Ok(encoder)
    }

    pub(in crate::encoding) fn row_metrics(
        &self,
    ) -> Result<(usize, usize, usize), PackageReviewEncodingError> {
        self.check()?;
        Ok((
            self.encoded_bytes,
            self.text.as_ref().map_or(0, Writer::length),
            self.policy_elements.expect("row encoder bounds elements"),
        ))
    }

    pub(in crate::encoding) fn finish_row(
        self,
    ) -> Result<(Vec<u8>, String), PackageReviewEncodingError> {
        self.check()?;
        Ok((
            self.output,
            self.text
                .map(Writer::finish)
                .transpose()?
                .unwrap_or_default(),
        ))
    }
}
