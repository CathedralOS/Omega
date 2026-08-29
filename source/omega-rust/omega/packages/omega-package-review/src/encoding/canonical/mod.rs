//! Canonical package-review framing and semantic row encoding.

mod declarations;
mod encoder;
mod review;
mod rows;

#[allow(unused_imports)]
pub(crate) use declarations::{
    encode_conformance_bound, encode_conformance_shape, encode_dangerous_authority,
    encode_dangerous_authority_slack, encode_data_field, encode_data_member,
    encode_data_properties, encode_data_shape, encode_domain_alias_atom,
    encode_domain_establishment_route, encode_domain_shape, encode_machine_parameter_contract,
    encode_machine_parameter_signature, encode_optional_u64, encode_relevance,
    encode_representation_tcb, encode_semantic_dependency, encode_semantic_dependency_key,
    encode_trait_parent, encode_trait_requirement, encode_trait_shape, encode_type_identity,
    encode_type_parameter, semantic_dependency_kind_tag,
};
pub(crate) use encoder::Encoder;
#[allow(unused_imports)]
pub(crate) use review::{encode, encode_with_limits};
#[allow(unused_imports)]
pub(crate) use rows::{
    canonical_row_kind_tag, canonical_row_risk_tag, encode_row, encode_rows,
    encode_rows_with_limits, push_row, row_source,
};

pub(crate) const MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW\0";
pub const PACKAGE_REVIEW_ENCODING_VERSION: u16 = 81;
pub(crate) const ROW_MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-ROW\0";
pub const PACKAGE_REVIEW_ROW_ENCODING_VERSION: u16 = 39;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageReviewEncodingLimits {
    maximum_review_bytes: usize,
    maximum_rows: usize,
    maximum_row_key_bytes: usize,
    maximum_row_bytes: usize,
    maximum_total_row_bytes: usize,
}

impl PackageReviewEncodingLimits {
    pub const fn new(
        maximum_review_bytes: usize,
        maximum_rows: usize,
        maximum_row_key_bytes: usize,
        maximum_row_bytes: usize,
        maximum_total_row_bytes: usize,
    ) -> Self {
        Self {
            maximum_review_bytes,
            maximum_rows,
            maximum_row_key_bytes,
            maximum_row_bytes,
            maximum_total_row_bytes,
        }
    }
}

impl Default for PackageReviewEncodingLimits {
    fn default() -> Self {
        Self::new(
            16 * 1024 * 1024,
            65_536,
            1024 * 1024,
            4 * 1024 * 1024,
            16 * 1024 * 1024,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewEncodingError {
    message: &'static str,
}

impl PackageReviewEncodingError {
    pub(crate) const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for PackageReviewEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for PackageReviewEncodingError {}
