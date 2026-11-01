//! Portable selected-opaque application vocabulary, independent of Source.
//!
//! The v1 commitment is shared by Build selection, Terminal publication and
//! native boundary review. Moving the encoder must not change its bytes.

use optimization::digest_encoder::DomainDigestEncoder;

pub const OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION: u16 = 1;

/// V1 carriers supply inert storage, not independently invoked lifecycle code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum OpaqueRepresentationLifecycleDisposition {
    Inert,
}

/// Relocation of one occurrence is distinct from a checked semantic copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum OpaqueRepresentationCopyDisposition {
    PlacementOnly,
    CheckedSemanticCopy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum OpaqueRepresentationApplicationOrigin {
    NamedConformance,
}

pub fn selected_application_commitment(
    conformance_application_commitment: [u8; 32],
    lifecycle: OpaqueRepresentationLifecycleDisposition,
    copy_disposition: OpaqueRepresentationCopyDisposition,
    origin: OpaqueRepresentationApplicationOrigin,
) -> [u8; 32] {
    let mut digest = DomainDigestEncoder::new(b"omega.opaque-representation-application.v1");
    digest.u16(OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION);
    digest.byte(match origin {
        OpaqueRepresentationApplicationOrigin::NamedConformance => 1,
    });
    digest.byte(match lifecycle {
        OpaqueRepresentationLifecycleDisposition::Inert => 1,
    });
    digest.byte(match copy_disposition {
        OpaqueRepresentationCopyDisposition::PlacementOnly => 1,
        OpaqueRepresentationCopyDisposition::CheckedSemanticCopy => 2,
    });
    digest.raw(&conformance_application_commitment);
    digest.finish().into()
}

/// Exact portable declaration roles and the complete existing Source-v4
/// application encoding. Descriptive custody, never selection authority.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct SelectedOpaqueApplication {
    pub opaque_identity: String,
    pub carrier_identity: String,
    pub conformance_identity: String,
    pub application_report_fingerprint: u64,
    pub conformance_application_commitment: [u8; 32],
    pub identity_encoding: Vec<u8>,
    pub schema_version: u16,
    pub origin: OpaqueRepresentationApplicationOrigin,
    pub lifecycle: OpaqueRepresentationLifecycleDisposition,
    pub copy_disposition: OpaqueRepresentationCopyDisposition,
    pub selected_application_commitment: [u8; 32],
}

impl SelectedOpaqueApplication {
    /// Ingress consistency of the complete existing protocol, not evidence
    /// that this application was selected by an admitted Build invocation.
    pub fn validate_identity_encoding(&self) -> Result<(), String> {
        if self.opaque_identity.is_empty()
            || self.carrier_identity.is_empty()
            || self.conformance_identity.is_empty()
            || self.identity_encoding.is_empty()
            || self.schema_version != OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION
        {
            return Err("opaque representation lost its complete selected application".into());
        }
        let (report, commitment) =
            crate::conformance_identity::source_identity_commitments(&self.identity_encoding);
        if report != self.application_report_fingerprint
            || commitment != self.conformance_application_commitment
        {
            return Err("opaque representation changed its Source-v4 identity preimage".into());
        }
        if selected_application_commitment(
            commitment,
            self.lifecycle,
            self.copy_disposition,
            self.origin,
        ) != self.selected_application_commitment
        {
            return Err(
                "opaque representation changed its complete selected-application commitment".into(),
            );
        }
        Ok(())
    }
}
