//! Closed carrier correspondence for caller-activation storage operations.
//!
//! The claim descriptor and the backing are different residents. Transport may
//! relocate the descriptor; no operation transports the backing out of the
//! activation. The establishment operation is also its published site row:
//! it retains the complete bound, chosen extent operand and conditional claim.

use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, StructuralAccess, StructuralMultiplicity,
    StructuralPathSegment, StructuralTypeShape, TerminalModule,
};

#[derive(Debug, Clone, Copy)]
pub struct ActivationClaimShape {
    pub claim_type: StructuralTypeId,
    pub claim_field: StructuralFieldId,
}

pub fn activation_claim_shape(
    module: &TerminalModule,
    operation: &Operation,
) -> Result<ActivationClaimShape, &'static str> {
    activation_claim_shape_in(&module.structural_types, operation)
}

pub fn activation_claim_shape_in(
    types: &[terminal_psi::StructuralTypeDeclaration],
    operation: &Operation,
) -> Result<ActivationClaimShape, &'static str> {
    let OperationKind::EstablishActivationClaim {
        bound,
        backing_type,
        established_case,
        rejected_case,
        ..
    } = operation.kind
    else {
        return Err("expected activation establishment");
    };
    let result = operation
        .result
        .structural()
        .ok_or("activation establishment requires a structural result")?;
    if result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
    {
        return Err("activation attempt carries unexpected custody or qualifications");
    }
    let backing = terminal_psi::find_structural_type(types, backing_type)
        .ok_or("activation backing type is missing")?;
    if backing.owns_linear_claim
        || !backing.no_code_affine_cleanup
        || backing.descriptor_cleanup
        || backing.quotient_carrier.is_some()
        || backing.scalar_restriction.is_some()
        || backing.shape
            != StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: bound,
            })
    {
        return Err("activation backing must reserve exactly the complete closed bound");
    }
    let attempt = terminal_psi::find_structural_type(types, result.structural_type)
        .ok_or("activation attempt type is missing")?;
    let StructuralTypeShape::Sum { cases } = &attempt.shape else {
        return Err("activation attempt is not a sum");
    };
    let [established, rejected] = cases.as_slice() else {
        return Err("activation attempt requires exactly two outcomes");
    };
    if established.id != established_case
        || rejected.id != rejected_case
        || established_case == rejected_case
        || !rejected.fields.is_empty()
        || attempt.owns_linear_claim
        || attempt.quotient_carrier.is_some()
    {
        return Err("activation attempt outcome roster differs from its operation");
    }
    let [field] = established.fields.as_slice() else {
        return Err("established outcome requires exactly one claim");
    };
    let claim_type = field
        .field_type
        .as_structural()
        .ok_or("activation claim payload is not structural")?;
    let claim = terminal_psi::find_structural_type(types, claim_type)
        .ok_or("activation claim type is missing")?;
    if field.relevance.is_erased()
        || !claim.owns_linear_claim
        || claim.no_code_affine_cleanup
        || claim.descriptor_cleanup
        || claim.quotient_carrier.is_some()
        || claim.scalar_restriction.is_some()
        || claim.shape
            != StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::MutableBorrow),
            })
    {
        return Err("activation claim must be an opaque linear exclusive byte descriptor");
    }
    let [binding] = result.claims.as_slice() else {
        return Err("activation establishment requires one conditional claim binding");
    };
    if binding.path
        != [
            StructuralPathSegment::Case(established_case),
            StructuralPathSegment::Field(field.identity),
        ]
    {
        return Err("activation grant must occur only below its established outcome");
    }
    Ok(ActivationClaimShape {
        claim_type,
        claim_field: field.id,
    })
}

mod flow;
pub use flow::{validate_flow, validate_machine};
