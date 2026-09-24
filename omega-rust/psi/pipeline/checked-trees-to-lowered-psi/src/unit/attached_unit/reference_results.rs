//! Reference carriers retain ingress provenance and the exact source loan end.
use super::super::StructuralArgument;
use super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, LoweringError, Multiplicity, Operation, OperationKind,
    OperationResult, StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, allocate_dense, lookup_type_id, place_id, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralResultBindingPlan,
};

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
    let (typed_machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, machine.state)?;
    let returned = machine
        .structural_result
        .as_ref()
        .ok_or(LoweringError::Unsupported(
            "reference establishment has no returned carrier",
        ))?;
    let [reference] = returned.reference_sources.as_slice() else {
        return unsupported("reference completion source roster changed");
    };
    let parameter_index = match &source.source {
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }
        | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            parameter_index, ..
        }
        | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            parameter_index, ..
        } => *parameter_index,
        _ => return unsupported("reference establishment requires retained ingress custody"),
    };
    let parameter = machine
        .structural_parameters
        .get(parameter_index as usize)
        .ok_or(LoweringError::Unsupported(
            "reference ingress parameter is absent",
        ))?;
    // The checked plan carries the whole-ingress shape and the projected
    // leaf shape in the same fields; reconstructing the authored tail selects
    // exactly one.
    let exact_ingress = if let Some(position) =
        validation::reference_result_custody::source_parameter(&checked.typed, state)
    {
        parameter.position as usize == position
            && matches!(
                parameter.access,
                CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::SharedBorrow
            )
            && parameter.access == source.access
            && parameter.type_identity == source.type_identity
            && parameter.qualifications.is_empty()
            && source.path.is_empty()
    } else if let Some((position, expected)) =
        validation::reference_result_custody::source_leaf(&checked.typed, state)
    {
        parameter.position as usize == position
            && parameter.access == CheckedStructuralAccess::Owned
            && parameter.multiplicity == Multiplicity::Affine
            && parameter.qualifications.is_empty()
            && expected == *source
    } else if let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
        expression, ..
    }
    | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
        expression, ..
    } = source.source
    {
        let authored_position = (|| {
            let ExpressionNode::Indexed(indexed) =
                checked.typed.expression_table.expression(expression)
            else {
                return None;
            };
            let ExpressionNode::Name(path) = checked
                .typed
                .expression_table
                .expression(indexed.collection)
            else {
                return None;
            };
            if path.symbol != path.head_symbol
                || checked
                    .typed
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    != 1
            {
                return None;
            }
            checked
                .typed
                .state_parameters(state)
                .iter()
                .position(|authored| authored.symbol == path.symbol)
        })();
        let authored_tail = checked
            .typed
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .is_some_and(|statement| {
                matches!(
                    statement,
                    StatementNode::Expression(tail)
                        if *tail == expression
                )
            });
        Some(parameter.position as usize) == authored_position
            && parameter.access == CheckedStructuralAccess::SharedBorrow
            && parameter.access == source.access
            && parameter.type_identity == source.type_identity
            && parameter.qualifications.is_empty()
            && source.path.is_empty()
            && authored_tail
    } else if let CheckedUnitStructuralArgumentSourcePlan::Parameter { .. } = source.source {
        // A `collection[0..collection.len]` completion names the carrier
        // parameter itself — or, for `self.field[0..len]`, the projected
        // `&`-field's referent leaf — rather than a derived subslice place.
        let authored = (|| {
            let statements = checked
                .typed
                .statement_table
                .statements(state.statement_nodes);
            let StatementNode::Expression(tail) = statements.last()? else {
                return None;
            };
            let ExpressionNode::Indexed(indexed) = checked.typed.expression_table.expression(*tail)
            else {
                return None;
            };
            let ExpressionNode::Range(range) =
                checked.typed.expression_table.expression(indexed.index)
            else {
                return None;
            };
            if range.end_inclusive || !range.start.is_valid() || !range.end.is_valid() {
                return None;
            }
            let ExpressionNode::Integer(start) =
                checked.typed.expression_table.expression(range.start)
            else {
                return None;
            };
            if start.value_i64() != Some(0) {
                return None;
            }
            let receiver = validation::collection_length_receiver(
                &checked.typed,
                typed_machine,
                Some(state),
                range.end,
            );
            let receiver = receiver?;
            if !checked
                .typed
                .expression_table
                .expressions_structurally_equal(indexed.collection, receiver)
            {
                return None;
            }
            // The collection's root selects the carrier: a membered path
            // walks receivers down to the head `Name`; `self` is not a state
            // parameter, so a `None` authored position on a receiver root
            // falls back to the `is_self` structural formal below.
            let mut cursor = indexed.collection;
            let mut depth = 0usize;
            loop {
                match checked.typed.expression_table.expression(cursor) {
                    ExpressionNode::Name(path) => {
                        let position = checked
                            .typed
                            .state_parameters(state)
                            .iter()
                            .position(|authored| authored.symbol == path.head_symbol);
                        break Some((position, depth));
                    }
                    ExpressionNode::Member(member) => {
                        depth += 1;
                        cursor = member.receiver;
                    }
                    _ => break None,
                }
            }
        })();
        let Some((authored_position, member_depth)) = authored else {
            return unsupported("reference completion has no exact ingress source");
        };
        let authored_position = match authored_position {
            Some(position) => position,
            None if parameter.is_self => parameter.position as usize,
            None => {
                return unsupported("reference completion has no exact ingress source");
            }
        };
        // `x[0..x.len]` re-borrows the parameter itself (path stays empty);
        // `self.view[0..len]` names the stored `&` field's referent leaf, one
        // `Referent` segment past the field projections — the verifier's
        // projected carrier type independently enforces the leaf identity.
        let expected_path_len = if member_depth == 0 {
            0
        } else {
            member_depth + 1
        };
        let path_shape = source.path.len() == expected_path_len
            && source
                .path
                .iter()
                .take(member_depth)
                .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
            && (member_depth == 0
                || matches!(
                    source.path.last(),
                    Some(CheckedUnitStructuralPathSegment::Referent)
                ));
        parameter.position as usize == authored_position
            && parameter.access == CheckedStructuralAccess::SharedBorrow
            && parameter.access == source.access
            && parameter.multiplicity == Multiplicity::Unrestricted
            && parameter.qualifications.is_empty()
            && path_shape
            && (member_depth != 0 || parameter.type_identity == source.type_identity)
    } else {
        return unsupported("reference completion has no exact ingress source");
    };
    if !exact_ingress
        || !reference.path.is_empty()
        || reference.source != *source
        || binding.multiplicity != Multiplicity::Affine
        || returned.multiplicity != binding.multiplicity
        || returned.type_identity != binding.type_identity
        || returned.source
            != (CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: binding.binding_ordinal,
            })
        || binding.statement_index as usize + 1
            != checked
                .statement_table
                .statements(state.statement_nodes)
                .len()
        || !(checked.normalized_type_identity(state.return_type).as_str() == binding.type_identity
            || super::structural_completion::shared_borrowed_slice_referee_identity(
                checked,
                state.return_type,
            )
            .as_deref()
                == Some(binding.type_identity.as_str()))
    {
        return unsupported("reference completion differs from its exact returned ingress");
    }
    Ok(())
}

pub(super) fn validate_releases(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    for (index, operation) in machine.operations.iter().enumerate() {
        let CheckedUnitEffectOperationPlan::StructuralCall {
            custody, result, ..
        } = operation
        else {
            continue;
        };
        if !custody.reference_loan.is_valid() {
            continue;
        }
        super::structural_calls::validate_custody(
            checked,
            machine.machine,
            machine.state,
            operation,
        )?;
        let boundary = validation::reference_result_custody::release_statement(
            &checked.facts,
            machine.machine,
            machine.state,
            custody.reference_loan,
        )
        .ok_or(LoweringError::Unsupported(
            "reference carrier has no exact weakening boundary",
        ))?;
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
                super::scalar_arrays::source_statement(operation)
                    .is_some_and(|statement| statement >= boundary)
            })
        {
            return unsupported(
                "reference loan release follows an already resumed source statement",
            );
        }
    }
    for operation in &machine.operations {
        if let CheckedUnitEffectOperationPlan::ReleaseReference { binding_ordinal, loan, .. } = operation
            && !machine.operations.iter().any(|producer| matches!(producer,
                CheckedUnitEffectOperationPlan::StructuralCall { custody, result, .. }
                    if result.binding_ordinal == *binding_ordinal && custody.reference_loan == *loan && loan.is_valid()))
        {
            return unsupported("reference release has no retained result loan");
        }
    }
    Ok(())
}

pub(super) fn validate_consumer(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
    expression: checked_trees::expression::ExpressionHandle,
) -> Result<bool, LoweringError> {
    let Some(binding_ordinal) = argument.source_structural_result_binding_ordinal() else {
        return Ok(false);
    };
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller.state)?;
    let record_result = caller.operations.iter().find_map(|operation| match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
        | CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
            if result.binding_ordinal == binding_ordinal => Some(result),
        _ => None,
    }).filter(|result| {
        matches!(checked.statement_table.statements(state.statement_nodes).get(result.statement_index as usize),
            Some(StatementNode::LocalData(local)) if validation::reference_result_custody::is_reference_record(&checked.typed, local.type_reference))
    });
    if let Some(result) = record_result {
        let Some(reference) =
            validation::declared_place_type_raw(&checked.typed, machine, Some(state), expression)
        else {
            return Ok(false);
        };
        if validation::reference_result_custody::parts(&checked.typed, reference).is_none() {
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
    let Some(producer @ CheckedUnitEffectOperationPlan::StructuralCall { custody, result, .. }) = caller.operations.iter()
        .find(|operation| matches!(operation, CheckedUnitEffectOperationPlan::StructuralCall { result, .. } if result.binding_ordinal == binding_ordinal))
    else { return Ok(false); };
    if !custody.reference_loan.is_valid() {
        return Ok(false);
    }
    super::structural_calls::validate_custody(checked, caller.machine, caller.state, producer)?;
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller.state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("reference result consumer has no authored local");
    };
    let (referent, _) =
        validation::reference_result_custody::parts(&checked.typed, local.type_reference).ok_or(
            LoweringError::Unsupported("reference local lost its declared reference type"),
        )?;
    let boundary = validation::reference_result_custody::release_statement(
        &checked.facts,
        caller.machine,
        caller.state,
        custody.reference_loan,
    )
    .ok_or(LoweringError::Unsupported(
        "reference consumer has no weakening boundary",
    ))?;
    if argument.path != [CheckedUnitStructuralPathSegment::Referent]
        || argument.access != CheckedStructuralAccess::MutableBorrow
        || parameter.access != argument.access
        || argument.type_identity != parameter.type_identity
        || checked.normalized_type_identity(referent).as_str() != argument.type_identity
        || !parameter.qualifications.is_empty()
        || parameter.multiplicity != Multiplicity::Unrestricted
        || coordinate.statement_index <= result.statement_index
        || coordinate.statement_index >= boundary
        || !matches!(checked.expression_table.expression(expression), ExpressionNode::Name(path)
            if path.symbol == local.symbol && path.head_symbol == local.symbol
                && checked.expression_table.name_path_members(path.members).len() == 1)
    {
        return unsupported("reference consumer differs from its live captured referent");
    }
    Ok(true)
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
