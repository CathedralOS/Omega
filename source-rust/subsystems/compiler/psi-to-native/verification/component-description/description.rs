//! Read the component's published description and independently reconstruct its
//! entries, exports, authority and obligations from the embedded Psi artifact.
#![forbid(unsafe_code)]

pub mod inventory;
pub mod records;
pub mod retained_plan;
mod verification;
mod wire_format;

pub use records::{COMPONENT_DESCRIPTION_SCHEMA_V3, DescribeError};
pub use verification::{
    AcceptedComponentAssumptions, ComponentVerificationRejection, ComponentVerificationRequest,
    VerifiedComponent, verify_component,
};
pub use wire_format::{
    decode_component_description, decode_component_description_with_artifact, description_subject,
    encode_component_description, encode_component_description_checked,
};

#[cfg(test)]
mod tests;
