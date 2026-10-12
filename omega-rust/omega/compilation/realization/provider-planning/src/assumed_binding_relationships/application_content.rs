//! Portable content of the two original boundary requirement descriptions.
//!
//! The retained operand owner issues the canonical content; final Product arena
//! slots, local equality identities and representative executable applications
//! cannot donate it. This component is separate from declaration revisions,
//! consuming root scope, supplier closure and domain issuance. A nonexecuting
//! descriptor selecting an invocation lifetime family retains universal original
//! formal coordinates separately from actual supplied captured regions.
//! Neither unavailable content nor a produced pair changes source admission,
//! supplies receiving consent or installs an assumed relationship.

use crate::assumed_binding_registrations::ProductBoundaryRequirementReference;
use crate::digest_encoder::DomainDigestEncoder;

/// Construction belongs exclusively to the original retained operand projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingApplicationContent {
    required: [u8; 32],
    supplied: [u8; 32],
}

impl AssumedBindingApplicationContent {
    pub const fn required(self) -> [u8; 32] {
        self.required
    }

    pub const fn supplied(self) -> [u8; 32] {
        self.supplied
    }
}

pub(super) fn capture(
    required: &ProductBoundaryRequirementReference,
    supplied: &ProductBoundaryRequirementReference,
) -> Option<AssumedBindingApplicationContent> {
    Some(AssumedBindingApplicationContent {
        required: application_content(required)?,
        supplied: application_content(supplied)?,
    })
}

fn application_content(reference: &ProductBoundaryRequirementReference) -> Option<[u8; 32]> {
    let content = checked_trees::typed_trees::static_applications::requirement_description_application_identity_text(
        reference.application_owner(),
        reference.application_handle(),
    )
    .ok()?;
    if content.is_empty() {
        return None;
    }
    let mut encoder = DomainDigestEncoder::new(b"omega.assumed-binding-original-application.v1\0");
    encoder.bytes(content.as_bytes());
    Some(encoder.finish())
}
