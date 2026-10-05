//! Reference carriers retain ingress provenance and the exact source loan end.
use super::super::StructuralArgument;
use super::admission::CallerView;
use super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, LoweringError, Operation, OperationKind,
    OperationResult, StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, allocate_dense, lookup_type_id, place_id, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::CheckedUnitStructuralResultBindingPlan;
use checked_trees::statement::StatementNode;

pub(super) fn validate_establishment(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::EstablishReference {
        result: binding,
        source,
    } = operation
    else {
        return unsupported("reference establishment has no carrier producer");
    };
    let (_, state) = crate::declarations::state_declaration(checked, machine.state)?;
    if matches!(
        checked
            .statement_table
            .statements(state.statement_nodes)
            .get(binding.statement_index as usize),
        Some(StatementNode::LocalData(_))
    ) {
        return validate_local_establishment(checked, machine.state, binding);
    }
    let returned = machine
        .structural_result
        .as_ref()
        .ok_or(LoweringError::Unsupported(
            "reference establishment has no returned carrier",
        ))?;
    let [_] = returned.reference_sources.as_slice() else {
        return unsupported("reference completion source roster changed");
    };
    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = source.source
    else {
        return unsupported("reference establishment requires retained ingress custody");
    };
    if machine
        .structural_parameters
        .get(parameter_index as usize)
        .is_none()
    {
        return unsupported("reference ingress parameter is absent");
    }
    // The authored tail names the whole ingress, a projected leaf, or a
    // borrowed field loan.
    let borrowed_tail = || {
        let expression = crate::reference_custody::tail_return_expression(checked, state)?;
        validation::reference_result_custody::borrowed_initializer_source(
            &checked.typed,
            state,
            expression,
            state.return_type,
        )
    };
    if !crate::reference_custody::tail_names_source_parameter(checked, state)
        && !crate::reference_custody::tail_selects_source_leaf(checked, state)
        && borrowed_tail().is_none()
    {
        return unsupported("reference completion has no exact ingress source");
    }
    Ok(())
}

/// A local loan replays the exact authored primitive source and retains the
/// full reference shell independently of the backing carrier.
pub(super) fn validate_local_establishment(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let (_, authored) = crate::declarations::state_declaration(checked, state)?;
    let ordinal = result.statement_index as usize;
    if validation::reference_result_custody::primitive_local_source(
        &checked.typed,
        authored,
        ordinal,
    )
    .is_none()
    {
        return unsupported("primitive reference lost its authored source");
    }
    let Some(StatementNode::LocalData(_)) = checked
        .statement_table
        .statements(authored.statement_nodes)
        .get(ordinal)
    else {
        return unsupported("primitive reference has no local declaration");
    };
    Ok(())
}

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

/// Rejoin a composed reference completion to its authored source. Shared
/// returns reconstruct the same initializer custody as ordinary completions;
/// exclusive returns also retain the borrow checker's exact return path.
pub(super) fn validate_composed_establishment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    result: &CheckedUnitStructuralResultBindingPlan,
    source: &checked_trees::CheckedUnitStructuralArgumentPlan,
    expression: checked_trees::expression::ExpressionHandle,
) -> Result<(), LoweringError> {
    let (_, authored) = crate::declarations::state_declaration(checked, state.state)?;
    if crate::reference_custody::shared_borrow_parts(checked, authored.return_type).is_some() {
        // A shared result may weaken mutable ingress; its exact authored
        // initializer must resolve.
        if validation::reference_result_custody::initializer_source(
            &checked.typed,
            authored,
            expression,
            authored.return_type,
        )
        .is_none()
        {
            return unsupported("shared reference return has no exact ingress source");
        }
        return Ok(());
    }
    let Some(_) = validation::reference_result_custody::mutable_return_source(
        &checked.typed,
        &checked.facts,
        machine,
        authored,
        result.statement_index as usize,
        expression,
    ) else {
        return unsupported("mutable reference return has no exact ingress source");
    };
    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = source.source
    else {
        return unsupported("mutable reference return requires retained ingress custody");
    };
    if state
        .structural_parameters
        .get(parameter_index as usize)
        .is_none()
    {
        return unsupported("mutable reference ingress parameter is absent");
    }
    if crate::reference_custody::mutable_borrow_parts(checked, authored.return_type).is_none() {
        return unsupported("mutable reference return lost its declared referee");
    }
    Ok(())
}

pub(super) fn validate_releases(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    for (index, operation) in machine.operations.iter().enumerate() {
        // Only an ordinary structural call carries a reference loan.
        let Some((result, custody)) = ordinary_structural_result(operation) else {
            continue;
        };
        if !custody.reference_loan.is_valid() {
            continue;
        }
        let boundary = validation::reference_result_custody::release_boundary(
            &checked.typed,
            &checked.facts,
            machine.machine,
            machine.state,
            custody.reference_loan,
        )
        .ok_or(LoweringError::Unsupported(
            "reference carrier has no exact weakening boundary",
        ))?;
        let (_, authored) = crate::declarations::state_declaration(checked, machine.state)?;
        if boundary as usize
            >= checked
                .statement_table
                .statements(authored.statement_nodes)
                .len()
        {
            // The loan's last use is the final statement: the state exit
            // retires the custody itself, so no release operation exists.
            continue;
        }
        let mut releases = machine.operations.iter().enumerate().filter_map(
            |(index, operation)| match operation {
                CheckedUnitEffectOperationPlan::ReleaseReference {
                    binding_ordinal,
                    loan,
                    statement_index,
                } if *binding_ordinal == result.binding_ordinal => {
                    Some((index, *loan, *statement_index))
                }
                _ => None,
            },
        );
        let (release_index, loan, statement_index) = releases.next().ok_or(
            LoweringError::Unsupported("reference carrier omits its loan release"),
        )?;
        if releases.next().is_some()
            || release_index <= index
            || loan != custody.reference_loan
            || statement_index != boundary
        {
            return unsupported("reference carrier release differs from captured weakening");
        }
        // The boundary is before the first source operation at that statement,
        // never merely a matching coordinate emitted after a conflicting read.
        if machine.operations[index + 1..release_index]
            .iter()
            .any(|operation| {
                operation
                    .source_statement_index()
                    .is_some_and(|statement| statement >= boundary)
            })
        {
            return unsupported(
                "reference loan release follows an already resumed source statement",
            );
        }
    }
    for operation in &machine.operations {
        if let CheckedUnitEffectOperationPlan::ReleaseReference {
            binding_ordinal,
            loan,
            ..
        } = operation
            && !machine.operations.iter().any(|producer| {
                ordinary_structural_result(producer).is_some_and(|(result, custody)| {
                    result.binding_ordinal == *binding_ordinal
                        && custody.reference_loan == *loan
                        && loan.is_valid()
                })
            })
        {
            return unsupported("reference release has no retained result loan");
        }
    }
    Ok(())
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
    if let Some(result) = record_result {
        let Some(reference) =
            validation::declared_place_type_raw(&checked.typed, machine, Some(state), expression)
        else {
            return Ok(false);
        };
        if crate::reference_custody::primitive_loan_parts(checked, reference).is_none() {
            return Ok(false);
        }
        let expected = validation::reference_result_custody::record_argument(
            &checked.typed,
            &checked.facts,
            caller.machine,
            state,
            coordinate.statement_index,
            expression,
            result,
            reference,
        )
        .ok_or(LoweringError::Unsupported(
            "record reference consumer has no exact live source",
        ))?;
        if expected != *argument
            || argument.type_identity != parameter.type_identity
            || argument.access != parameter.access
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
                && validation::has_plain_owned_contents_with_represented_constraints(
                    &checked.typed,
                    referent,
                )
        });
    if !primitive && !plain_record {
        return unsupported("reference consumer has no plain record or primitive referent");
    }
    let loan = match producer {
        CheckedUnitEffectOperationPlan::EstablishReference { result, .. } => {
            validate_local_establishment(checked, caller.state, result)?;
            None
        }
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
    if let Some(loan) = loan {
        let boundary = validation::reference_result_custody::release_boundary(
            &checked.typed,
            &checked.facts,
            caller.machine,
            caller.state,
            loan,
        )
        .ok_or(LoweringError::Unsupported(
            "reference consumer has no weakening boundary",
        ))?;
        if coordinate.statement_index >= boundary
            || (argument.path.len() > 1 && !active_result_loan(checked, caller, coordinate, loan))
        {
            return unsupported("reference consumer is outside its captured loan lifetime");
        }
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
