//! Array payload identities live in the structural namespace, not scalar slots.
//!
//! Reserving a place does not establish its value. Only the constructor effect
//! in the selected evaluation path does so; Terminal independently checks that
//! this producer dominates each use. No array payload crosses a block parameter.
use super::super::{
    Operation, OperationKind, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueDeclaration,
    emit_direct_expression,
};
use super::{LoweringError, unsupported, validate_direct_parameter_types};
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::place_declarations::operation_result_declaration;
use crate::scalar_lowering::branch_states::prepared::LoweredScalarArrayConstruction;

/// A computed array is a fixed primitive array whose leaf count fits in u64.
pub(crate) fn validate_layout(
    mut current: StructuralTypeId,
    types: &[StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    let mut count = Some(1u64);
    let mut empty = false;
    for depth in 0..types.len() {
        let declaration = terminal_psi::find_structural_type(types, current).ok_or(
            LoweringError::Unsupported("computed array element type is absent"),
        )?;
        match declaration.shape {
            StructuralTypeShape::FixedArray { element, length } => {
                // A later empty dimension suppresses product overflow, never
                // validation of the complete type beneath that dimension.
                empty |= length == 0;
                count = count.and_then(|count| count.checked_mul(length));
                current = element;
            }
            StructuralTypeShape::PrimitiveScalar(_) if depth != 0 => {
                if !empty && count.is_none() {
                    return unsupported("computed array leaf count overflows");
                }
                return Ok(());
            }
            _ => return unsupported("computed array requires a fixed primitive array shape"),
        }
    }
    unsupported("computed array type is cyclic")
}

pub(crate) fn emit(
    effects: &[LoweredScalarArrayConstruction],
    domains: &[crate::terminal_identities::StructuralDomainBinding],
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    let types = values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    for effect in effects {
        let mut elements = Vec::with_capacity(effect.elements.len());
        for element in &effect.elements {
            validate_direct_parameter_types(element, &types)?;
            if !matches!(element, LoweredDirectExpression::Parameter { .. }) {
                return unsupported("array construction requires completed scalar leaves");
            }
            elements.push(emit_direct_expression(
                element, values, next_value, operations,
            ));
        }
        let mut qualifications = effect
            .memberships
            .iter()
            .map(|domain| {
                crate::terminal_identities::lookup_domain_id_for_carrier(
                    domains,
                    *domain,
                    effect.structural_type,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        qualifications.sort();
        qualifications.dedup();
        let mut result = structural_result_declaration(
            effect.place,
            effect.structural_type,
            StructuralMultiplicity::Unrestricted,
        );
        if let terminal_psi::OperationResult::Structural(result) = &mut result {
            result.qualifications = qualifications;
        }
        operations.push_leaf(result, OperationKind::EstablishScalarArray { elements });
    }
    Ok(())
}

pub(crate) fn declarations(
    operations: &[Operation],
) -> impl Iterator<Item = StructuralPlaceDeclaration> + '_ {
    operations.iter().filter_map(|operation| {
        if !matches!(operation.kind, OperationKind::EstablishScalarArray { .. }) {
            return None;
        }
        operation_result_declaration(operation)
    })
}
