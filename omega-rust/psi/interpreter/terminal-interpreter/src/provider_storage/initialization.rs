//! Join a staged whole-value image to its exact admitted allocation window.
//!
//! The selected profile call describes the requested supply; the embedding
//! allocation's admission bounds its physical authority. Both must support the
//! complete write and subsequent view. A selected provider cannot amplify the
//! allocation's rights by returning more permissive ordinary profile data.
//! Preparation changes no bytes, descriptor, qualification or custody. Its
//! dynamic rejection is intended for the ordinary Rejected result carrying
//! the unchanged inputs, not a host error or a replacement extent.

use super::{ProviderStorageBinding, ProviderStorageSlot, RuntimeProviderStorage};
use crate::errors::{TerminalInterpretError, malformed};
use crate::primitive_storage::PrimitiveStorageEndpoint;
use crate::record::storage::ConstructorStorage;
use crate::values::{StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue};
use access_plans::{
    InitializationResourceCompatibility, ValidatedPlacementPlan, ValidatedResourceProfile,
};
use semantic_vocabulary::IntegerValue;
use terminal_psi::{StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralPathSegment};

#[derive(Debug)]
pub(crate) enum InitializationRejection {
    RangeTooSmall { required: u64, available: u64 },
    Misaligned { modulus: u64, residue: u64 },
    IncompatibleSupply,
}

/// This is only staged storage. Publishing Resident additionally requires the
/// complete original value's ownership transaction. There is deliberately no
/// standalone write method that could commit bytes ahead of that transaction.
pub(crate) struct PreparedProviderImage {
    pub(super) binding: ProviderStorageBinding,
    pub(super) image: Vec<u8>,
    /// Subsequent accesses specialize from actual admitted backing, never
    /// from the selected call's possibly more permissive profile description.
    pub(super) compatibility: InitializationResourceCompatibility,
}

impl RuntimeProviderStorage {
    pub(crate) fn prepare_initial_image(
        &mut self,
        binding: ProviderStorageBinding,
        supplied: ValidatedResourceProfile,
        placement: &ValidatedPlacementPlan,
        image: Vec<u8>,
    ) -> Result<Result<PreparedProviderImage, InitializationRejection>, TerminalInterpretError>
    {
        let required = placement.layout().size.ok_or_else(malformed)?;
        if u64::try_from(image.len()).ok() != Some(required) {
            return Err(malformed());
        }
        let ProviderStorageSlot::Live {
            allocation,
            profile,
            ..
        } = self.allocations.get_mut(binding.allocation)
        else {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "initialization allocation retired before staging",
            ));
        };
        let end = binding
            .offset
            .checked_add(binding.length)
            .ok_or_else(malformed)?;
        if end > allocation.extent.length()
            || usize::try_from(end)
                .ok()
                .is_none_or(|end| end > allocation.bytes.len())
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "initialization window exceeds its original allocation",
            ));
        }
        if required > binding.length {
            return Ok(Err(InitializationRejection::RangeTooSmall {
                required,
                available: binding.length,
            }));
        }
        let Ok(compatibility) =
            access_plans::validate_initialization_resources(placement, &supplied)
        else {
            return Ok(Err(InitializationRejection::IncompatibleSupply));
        };
        let base = allocation
            .extent
            .base()
            .checked_add(binding.offset)
            .ok_or_else(malformed)?;
        let congruence = compatibility.placement().base_congruence();
        if !congruence.admits(base) {
            return Ok(Err(InitializationRejection::Misaligned {
                modulus: congruence.modulus(),
                residue: congruence.residue(),
            }));
        }
        let Ok(loan) = allocation.extent.loan_mut(binding.offset, binding.length) else {
            return Ok(Err(InitializationRejection::IncompatibleSupply));
        };
        let Ok(admitted) = profile.restrict_to_loan(&loan) else {
            return Ok(Err(InitializationRejection::IncompatibleSupply));
        };
        let Ok(compatibility) =
            access_plans::validate_initialization_resources(placement, &admitted)
        else {
            return Ok(Err(InitializationRejection::IncompatibleSupply));
        };
        // Each profile may select a different implementation of the same
        // declared access. Both impose alignment, but only the admitted
        // allocation decides which physical operation is available. For
        // example, an External read cannot become a Stable read merely
        // because the selected provider described Stable supply.
        let congruence = compatibility.placement().base_congruence();
        if !congruence.admits(base) {
            return Ok(Err(InitializationRejection::Misaligned {
                modulus: congruence.modulus(),
                residue: congruence.residue(),
            }));
        }
        Ok(Ok(PreparedProviderImage {
            binding,
            image,
            compatibility,
        }))
    }
}

/// Stage the rejected outcome's failure case and its integer coordinate
/// payload. The rejection selects the contract's declared failure case and
/// carries the coordinates its fields report. Both outcome builders — the
/// initialization result and a borrowed placement's rejected result — stage
/// the same payload, so one owner derives it here.
pub(crate) fn stage_rejection_failure(
    storage: &mut ConstructorStorage,
    rejection: InitializationRejection,
    failures: &[StructuralCaseDeclaration; 3],
    reason_field: &StructuralFieldDeclaration,
    opaque_identity: u64,
) -> Result<(), TerminalInterpretError> {
    let (failure, coordinates) = match rejection {
        InitializationRejection::RangeTooSmall {
            required,
            available,
        } => (&failures[0], vec![required, available]),
        InitializationRejection::Misaligned { modulus, residue } => {
            (&failures[1], vec![modulus, residue])
        }
        InitializationRejection::IncompatibleSupply => (&failures[2], Vec::new()),
    };
    let reason = StructuralRuntimePlace {
        opaque_identity,
        path: vec![StructuralPathSegment::Field(reason_field.identity)],
    };
    storage.stage_case(
        reason.clone(),
        reason_field
            .field_type
            .as_structural()
            .ok_or_else(malformed)?,
        failure.id,
    );
    for (field, coordinate) in failure.fields.iter().zip(coordinates) {
        storage.stage_scalar_endpoint(
            &PrimitiveStorageEndpoint::ScalarField(StructuralScalarRuntimeField::case_payload(
                reason.clone(),
                field.id,
            )),
            TerminalScalarValue::Integer {
                scalar_type: field
                    .field_type
                    .scalar_type()
                    .and_then(|scalar| scalar.as_integer())
                    .ok_or_else(malformed)?,
                value: IntegerValue::Unsigned(u128::from(coordinate)),
            },
        )?;
    }
    Ok(())
}
