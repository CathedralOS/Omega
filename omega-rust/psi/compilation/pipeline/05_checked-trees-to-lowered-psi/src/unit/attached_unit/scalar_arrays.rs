//! Array construction shares the ordinary structural result namespace. Replay
//! rejoins each operand to its authored expression before emitting portable values.
//! Completion uses the sibling structural-completion checker; this module owns
//! array shape, storage and operand correspondence.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, Operation, OperationKind,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId,
    ValueDeclaration, allocate_dense, lookup_type_id, place_id, unsupported,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
use crate::expression_preparation::source_custody::array_sources::construction_expression;
use checked_trees::CheckedArrayConstructionSource;
use checked_trees::{CheckedCallScalarArgument, CheckedUnitStructuralResultBindingPlan};

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    source: CheckedArrayConstructionSource,
    result: &CheckedUnitStructuralResultBindingPlan,
    elements: &[CheckedCallScalarArgument],
) -> Result<(), LoweringError> {
    let (expression, reference) =
        construction_expression(checked, machine, state, result.statement_index, source).ok_or(
            LoweringError::Unsupported("array constructor does not rejoin its source owner"),
        )?;
    validate_shape(checked, reference)?;
    let array = validation::scalar_array_elements(&checked.typed, machine, expression, reference)
        .ok_or(LoweringError::Unsupported(
        "array constructor source is not an exact primitive array",
    ))?;
    for projection in array.projections {
        if let Some(selected) = checked.facts.operators.expression_use(projection)
            && (selected.spelling != language_core::OperatorSpelling::Index
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
        {
            return unsupported("array constructor indexing selection changed");
        }
    }
    let leaves = array.elements;
    if leaves.len() != elements.len() {
        return unsupported("array constructor scalar operand count changed");
    }
    for (ordinal, element) in elements.iter().enumerate() {
        let element_ordinal = u32::try_from(ordinal)
            .map_err(|_| LoweringError::Unsupported("array element ordinal exceeds u32"))?;
        let role = CheckedScalarExpressionRole::ArrayElement {
            source,
            element_ordinal,
        };
        match element {
            CheckedCallScalarArgument::Pure(_) => {
                if checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(state, result.statement_index, role)
                    .is_none()
                {
                    return unsupported("array element has no exact pure source binding");
                }
                if checked
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .any(|(_, root)| {
                        root.state == state
                            && root.statement_ordinal == result.statement_index
                            && root.role == role
                    })
                {
                    return unsupported("array element has conflicting pure and computed owners");
                }
            }
            CheckedCallScalarArgument::Computation(_) => {
                if !checked
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .any(|(_, root)| {
                        root.state == state
                            && root.statement_ordinal == result.statement_index
                            && root.role == role
                    })
                {
                    return unsupported("array element has no exact computation root");
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_shape(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> Result<(), LoweringError> {
    for _ in 0..checked.typed.type_reference_table.type_reference_count() {
        let identity = checked.typed.normalized_type_identity(reference);
        let mut matches = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .filter(|plan| plan.identity == identity.as_str());
        match matches.next() {
            Some(_) => {}
            None => {
                // A scalar qualification is a value-domain shell, not a
                // storage carrier: an element spelled `i32 in Saturating`
                // stores under the bare carrier's shape, so walk through the
                // shell the way custody checks do.
                match checked.typed.unwrapped_type_reference(reference) {
                    Some(carrier) if carrier != reference => {
                        reference = carrier;
                        continue;
                    }
                    _ => return unsupported("array source shape is absent"),
                }
            }
        }
        if matches.next().is_some() {
            return unsupported("array source shape identity is duplicated");
        }
        match checked.typed.type_reference_table.type_reference(reference) {
            checked_trees::types::TypeReferenceNode::FixedArray {
                element_type,
                length: checked_trees::types::FixedArrayLength::Literal(_),
            } => reference = *element_type,
            _ => return Ok(()),
        }
    }
    unsupported("array source carrier is cyclic")
}

pub(super) fn emit(
    result: &CheckedUnitStructuralResultBindingPlan,
    elements: &[ValueDeclaration],
    types: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let structural_type = lookup_type_id(types, &result.type_identity)?;
    let id = operations.allocate();
    let place = place_id(allocate_dense(next_place)?);
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result: structural_result_declaration(
            place,
            structural_type,
            StructuralMultiplicity::Unrestricted,
        ),
        kind: OperationKind::EstablishScalarArray {
            elements: elements.iter().map(|element| element.id).collect(),
        },
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type,
        },
    })
}
