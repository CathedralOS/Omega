//! Generated private backing carriers are prepared before callable bodies borrow
//! the shared type roster. They describe frame geometry, never claim authority.

use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::{
    CheckedUnitCallRoute, CheckedUnitEffectOperationPlan, CheckedUnitLocalBindingPlan,
};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalIdentityTableBuilder,
};

fn identity(bound: u64) -> String {
    format!("compiler(activation-byte-backing:{bound})")
}

pub(crate) fn backing_type(
    identities: &mut TerminalIdentityTableBuilder,
    types: &[StructuralTypeDeclaration],
    bound: u64,
) -> Result<StructuralTypeId, LoweringError> {
    let key = identities.intern(&identity(bound));
    let declaration = terminal_psi::structural_type_by_identity(types, key).ok_or(
        LoweringError::Unsupported("activation backing carrier was not prepared"),
    )?;
    if declaration.shape
        != StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: bound })
        || declaration.owns_linear_claim
        || !declaration.no_code_affine_cleanup
        || declaration.descriptor_cleanup
    {
        return unsupported("activation backing identity has conflicting geometry or custody");
    }
    Ok(declaration.id)
}

pub(crate) fn prepare_operation(
    identities: &mut TerminalIdentityTableBuilder,
    types: &mut Vec<StructuralTypeDeclaration>,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    if let Some(call) = operation.call()
        && let CheckedUnitCallRoute::ActivationStorage {
            bound: Some(bound), ..
        } = call.route()
    {
        let key = identities.intern(&identity(*bound));
        if terminal_psi::structural_type_by_identity(types, key).is_none() {
            let next = types
                .iter()
                .map(|row| row.id.get())
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "activation backing type identity overflows",
                ))?;
            types.push(StructuralTypeDeclaration {
                quotient_carrier: None,
                id: crate::terminal_identities::structural_type_id(next),
                identity: key,
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: *bound,
                }),
            });
        }
        backing_type(identities, types, *bound)?;
    }
    if let Some((_, calls)) = operation.structural_value_evaluation() {
        for call in calls {
            for dependency in call.operation_dependencies() {
                prepare_operation(identities, types, dependency)?;
            }
        }
    }
    Ok(())
}
