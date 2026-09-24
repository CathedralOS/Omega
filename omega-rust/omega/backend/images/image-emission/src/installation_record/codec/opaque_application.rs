//! By-value opaque boundary application custody: one row per edge carrying
//! the canonical requirement identity, the signature shape coordinate, the
//! compact report fingerprint, and the strong 256-bit selected-application
//! commitment the artifact must replay at bind.

use boundary_applications::{
    BoundaryOpaqueRepresentationApplication, BoundaryOpaqueRepresentationApplications,
};

use crate::installation_record::{InstallationError, Reader, push_u16, push_u32, push_u64};

pub(crate) fn encode_boundary_opaque_applications(
    bytes: &mut Vec<u8>,
    applications: &BoundaryOpaqueRepresentationApplications,
) {
    push_u32(
        bytes,
        u32::try_from(applications.rows().len())
            .expect("opaque application custody fits the record encoding"),
    );
    for row in applications.rows() {
        push_u32(
            bytes,
            u32::try_from(row.requirement_identity.len())
                .expect("opaque requirement identity fits the record encoding"),
        );
        bytes.extend_from_slice(row.requirement_identity.as_bytes());
        push_u16(bytes, row.shape_root);
        push_u64(bytes, row.application_report_fingerprint);
        bytes.extend_from_slice(&row.selected_application_commitment);
    }
}

pub(crate) fn decode_boundary_opaque_applications(
    reader: &mut Reader<'_>,
) -> Result<BoundaryOpaqueRepresentationApplications, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyBoundaryOpaqueApplications)?;
    // One row is at least 4 (length) + 2 + 8 + 32 bytes before its identity.
    if count > reader.remaining() / 46 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        let identity_len = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::BoundaryOpaqueApplicationIdentityTooLong)?;
        let requirement_identity = std::str::from_utf8(reader.take(identity_len)?)
            .map_err(|_| InstallationError::InvalidBoundaryOpaqueApplicationIdentity)?
            .to_owned();
        if requirement_identity.is_empty() {
            return Err(InstallationError::InvalidBoundaryOpaqueApplicationIdentity);
        }
        let shape_root = reader.u16()?;
        let application_report_fingerprint = reader.u64()?;
        let selected_application_commitment = reader.array::<32>()?;
        rows.push(BoundaryOpaqueRepresentationApplication {
            requirement_identity,
            shape_root,
            application_report_fingerprint,
            selected_application_commitment,
        });
    }
    BoundaryOpaqueRepresentationApplications::new(rows)
        .map_err(InstallationError::InvalidBoundaryOpaqueApplicationCustody)
}
