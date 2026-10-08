//! The installation record wire format.
//!
//! `envelope` frames the whole record and delegates each family to its codec;
//! `wire` owns the primitive reader and writers. The remaining names divide by
//! what the rows describe: `structural` and `scalar` for the two value worlds,
//! `unit` and `internal_unit` for a unit body's own rows and the calls it
//! makes, and `function` for what one installed function owns. A leaf module
//! encodes and decodes exactly one family of rows.

pub(super) mod boundary_result_scalar;
pub(super) mod boundary_settlement;
pub(super) mod call_site_owner;
pub(super) mod completion_custody;
pub(super) mod envelope;
pub(super) mod fingerprint;
pub(super) mod function;
pub(super) mod installation_header;
pub(super) mod internal_unit;
pub(super) mod mixed_structural_scalar_abi;
pub(super) mod opaque_application;
pub(super) mod parameter_abi;
pub(super) mod port_effect;
pub(super) mod private_function;
pub(super) mod provider_execution;
pub(super) mod provider_plan;
pub(super) mod scalar;
pub(super) mod semantic_code_attribution;
pub(super) mod structural;
pub(super) mod trivial_affine_local;
pub(super) mod unit;
pub(super) mod value_placement;
pub(super) mod wire;

/// The tag arms every structural-path decoder shares; tags the wire does not
/// assign return `None` so each caller keeps its own unknown-tag diagnostic.
pub(crate) fn scalar_path_segment(
    tag: u8,
    reader: &mut crate::installation_record::Reader<'_>,
) -> Result<
    Option<terminal_psi::StructuralPathSegment>,
    crate::installation_record::InstallationError,
> {
    use crate::installation_record::InstallationError;
    use terminal_psi::StructuralPathSegment;
    Ok(Some(match tag {
        2 => StructuralPathSegment::FixedIndex(reader.u64()?),
        3 => StructuralPathSegment::Referent,
        4 => StructuralPathSegment::FixedByteRange {
            start: reader.u64()?,
            end: reader.u64()?,
        },
        6 => StructuralPathSegment::RuntimeIndex {
            index: semantic_vocabulary::ValueId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroSettlementIdentity("ValueId"))?,
            obligation: semantic_vocabulary::ObligationId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroSettlementIdentity("ObligationId"))?,
        },
        7 => StructuralPathSegment::Case(
            semantic_vocabulary::StructuralCaseId::new(reader.u64()?).ok_or(
                InstallationError::ZeroSettlementIdentity("StructuralCaseId"),
            )?,
        ),
        _ => return Ok(None),
    }))
}
