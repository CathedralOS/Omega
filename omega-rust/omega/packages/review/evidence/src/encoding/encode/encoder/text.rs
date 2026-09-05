use super::{Encoder, PackageReviewEncodingError};
use crate::record::PackagePolicyBaseline;

mod framing;
mod writer;
pub(in crate::encoding) use framing::{HEADER, MAXIMUM_MARKUP_DEPTH, label};
pub(in crate::encoding) use writer::Writer;

pub const PACKAGE_POLICY_TEXT_VERSION: u16 = 1;
pub(in crate::encoding) const MAXIMUM_TEXT_BYTES: usize = 32 * 1024 * 1024;

impl PackagePolicyBaseline {
    /// Complete named comparison meaning, not a receipt of review or acceptance.
    pub fn canonical_text(&self) -> Result<String, PackageReviewEncodingError> {
        self.canonical_text_with_element_count()
            .map(|(text, _)| text)
    }

    /// Return the same text and the aggregate sequence/recursive-entry charge
    /// from its single encoding traversal. Enclosing documents can account for
    /// multiple baselines without resetting their element ceiling.
    pub fn canonical_text_with_element_count(
        &self,
    ) -> Result<(String, usize), PackageReviewEncodingError> {
        self.validate_canonical_structure()
            .map_err(PackageReviewEncodingError::new)?;
        render_with_element_count(self, Writer::new(MAXIMUM_TEXT_BYTES, None))
    }
}

pub(in crate::encoding) fn render(
    policy: &PackagePolicyBaseline,
    writer: Writer<'_>,
) -> Result<String, PackageReviewEncodingError> {
    render_with_element_count(policy, writer).map(|(text, _)| text)
}

fn render_with_element_count(
    policy: &PackagePolicyBaseline,
    writer: Writer<'_>,
) -> Result<(String, usize), PackageReviewEncodingError> {
    let mut encoder = Encoder::policy_text(writer);
    super::super::baseline::framed_policy(&mut encoder, policy)?;
    let remaining = encoder
        .policy_elements
        .expect("policy text bounds aggregate elements");
    let elements = crate::encoding::PackagePolicyRecoveryLimits::default()
        .maximum_sequence_elements
        - remaining;
    encoder.finish_text().map(|text| (text, elements))
}

impl<'text> Encoder<'text> {
    pub(in crate::encoding) fn policy_text(writer: Writer<'text>) -> Self {
        Self {
            text: Some(writer),
            ..Self::policy_bounded(4 * 1024 * 1024)
        }
    }

    pub(in crate::encoding) fn finish_text(self) -> Result<String, PackageReviewEncodingError> {
        self.check()?;
        self.text
            .ok_or_else(|| {
                PackageReviewEncodingError::new("binary encoder requires binary completion")
            })?
            .finish()
    }

    pub(crate) fn field<T>(
        &mut self,
        name: &'static str,
        encode: impl FnOnce(&mut Self) -> Result<T, PackageReviewEncodingError>,
    ) -> Result<T, PackageReviewEncodingError> {
        self.scope("field", name, encode)
    }

    pub(crate) fn record<T>(
        &mut self,
        name: &'static str,
        encode: impl FnOnce(&mut Self) -> Result<T, PackageReviewEncodingError>,
    ) -> Result<T, PackageReviewEncodingError> {
        self.scope("record", name, encode)
    }

    fn scope<T>(
        &mut self,
        kind: &'static str,
        name: &'static str,
        encode: impl FnOnce(&mut Self) -> Result<T, PackageReviewEncodingError>,
    ) -> Result<T, PackageReviewEncodingError> {
        if let Some(text) = &mut self.text {
            text.scope(kind, name);
        }
        let result = encode(self);
        if let Some(text) = &mut self.text {
            text.end();
            if result.is_err() {
                text.fail("package policy text field encoding failed");
            }
        }
        result.and_then(|value| self.check().map(|()| value))
    }
}
