//! Reference carriers retain ingress provenance and the exact source loan end.
use super::super::StructuralArgument;
use super::admission::CallerView;
use super::{
    CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError, Operation, OperationKind,
    OperationResult, StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, allocate_dense, lookup_type_id, place_id, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::CheckedUnitStructuralResultBindingPlan;
use checked_trees::statement::StatementNode;

/// An ordinary structural call's result binding and custody.
fn ordinary_structural_result(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<(
    &checked_trees::CheckedUnitStructuralResultBindingPlan,
    &checked_trees::CheckedStructuralCallCustodyPlan,
)> {
    let call = operation.call().filter(|call| !call.is_boundary())?;
    Some((call.structural_result()?, call.structural_custody()?))
}

pub(super) fn validate_consumer(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
    expression: checked_trees::expression::ExpressionHandle,
) -> Result<bool, LoweringError> {
    let Some(binding_ordinal) = argument.source_structural_result_binding_ordinal() else {
        return Ok(false);
    };
    let (machine, state) = crate::declarations::state_declaration(checked, caller.state)?;
    // A reference record is a constructed value or an ordinary structural
    // call's result.
    let record_result = caller.operations.iter().find_map(|operation| match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
            if result.binding_ordinal == binding_ordinal => Some(result),
        _ => ordinary_structural_result(operation)
            .map(|(result, _)| result)
            .filter(|result| result.binding_ordinal == binding_ordinal),
    }).filter(|result| {
        matches!(checked.statement_table.statements(state.statement_nodes).get(result.statement_index as usize),
            Some(StatementNode::LocalData(local)) if crate::reference_custody::is_reference_record(checked, local.type_reference))
    });
    if record_result.is_some() {
        let Some(reference) =
            validation::declared_place_type_raw(&checked.typed, machine, Some(state), expression)
        else {
            return Ok(false);
        };
        if crate::reference_custody::primitive_loan_parts(checked, reference).is_none() {
            return Ok(false);
        }
        if argument.type_identity != parameter.type_identity || argument.access != parameter.access
        {
            return unsupported("record reference consumer changed type or access");
        }
        return Ok(true);
    }
    // A stored reference's consumer depends on its declared carrier and live
    // producer, not on whether formation or a call published that carrier.
    let mut producers = caller.operations.iter().filter_map(|operation| {
        operation
            .structural_result_binding()
            .filter(|result| result.binding_ordinal == binding_ordinal)
            .map(|result| (operation, result))
    });
    let Some((producer, result)) = producers.next() else {
        return Ok(false);
    };
    if producers.next().is_some() {
        return unsupported("reference consumer has ambiguous producer custody");
    }
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return Ok(false);
    };
    let Some((referent, _)) =
        crate::reference_custody::stored_loan_parts(checked, local.type_reference)
    else {
        return Ok(false);
    };
    // Descriptor and erased-alias operands retain their existing representation
    // joins. This route crosses a stored Reference shell of the declared type.
    if result.type_identity
        != checked
            .normalized_type_identity(local.type_reference)
            .as_str()
    {
        return Ok(false);
    }
    let primitive = checked.primitive_type_reference(referent).is_some();
    let plain_record = checked
        .typed
        .closed_data_definition(referent)
        .is_some_and(|data| {
            checked
                .data_members(data)
                .iter()
                .all(|member| matches!(member, checked_trees::data::DataMember::Field(_)))
                && crate::content_classes::plain_owned_with_represented_constraints(
                    checked, referent,
                )
        });
    if !primitive && !plain_record {
        return unsupported("reference consumer has no plain record or primitive referent");
    }
    let loan = match producer {
        CheckedUnitEffectOperationPlan::EstablishReference { .. } => None,
        _ => {
            let Some((_, custody)) = ordinary_structural_result(producer) else {
                return unsupported("reference carrier producer has no retained custody");
            };
            if !custody.reference_loan.is_valid() {
                return unsupported("reference carrier producer lost its source loan");
            }
            Some(custody.reference_loan)
        }
    };
    if !caller.operations.iter().any(|operation| {
        operation
            .call()
            .is_some_and(|call| call.coordinate == coordinate)
    }) {
        return unsupported("reference consumer has no exact call occurrence");
    }
    if let Some(loan) = loan
        && argument.path.len() > 1
        && !active_result_loan(checked, caller, coordinate, loan)
    {
        return unsupported("reference consumer is outside its captured loan lifetime");
    }
    Ok(true)
}

fn active_result_loan(
    checked: &CheckedTrees,
    caller: &CallerView<'_>,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    loan: arena::Handle<checked_trees::BorrowLoanFact>,
) -> bool {
    let flow = &checked.facts.flow;
    let mut states = flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| {
            state.machine_symbol == caller.machine && state.state_symbol == caller.state
        });
    let Some(state) = states.next() else {
        return false;
    };
    if states.next().is_some() {
        return false;
    }
    let Some(statements) = flow.control.statements.span(state.statements) else {
        return false;
    };
    let mut entries = statements
        .iter()
        .filter(|statement| statement.statement_index == coordinate.statement_index as usize);
    let Some(entry) = entries.next() else {
        return false;
    };
    entries.next().is_none()
        && flow
            .contexts
            .constraint_refs
            .span(entry.entry_constraints)
            .is_some_and(|constraints| {
                constraints
                    .iter()
                    .filter(|constraint| {
                        matches!(constraint.kind,
            checked_trees::FlowConstraintKind::BorrowLoan { loan: active } if active == loan)
                    })
                    .count()
                    == 1
            })
}

pub(super) fn emit(
    result: &CheckedUnitStructuralResultBindingPlan,
    source: StructuralArgument,
    types: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let structural_type = lookup_type_id(types, &result.type_identity)?;
    emit_carrier(structural_type, source, next_place, operations)
}

pub(super) fn emit_carrier(
    structural_type: StructuralTypeId,
    source: StructuralArgument,
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let id = operations.allocate();
    let place = place_id(allocate_dense(next_place)?);
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishReference { source },
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type,
        },
    })
}
