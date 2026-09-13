//! Reference carriers retain ingress provenance and the exact source loan end.

use super::*;
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
    let (_, state) = crate::scalar_source_custody::authored_state(checked, machine.state)?;
    let position = validation::reference_result_custody::source_parameter(&checked.typed, state)
        .ok_or(LoweringError::Unsupported(
            "reference completion has no exact ingress source",
        ))?;
    let returned = machine
        .structural_result
        .as_ref()
        .ok_or(LoweringError::Unsupported(
            "reference establishment has no returned carrier",
        ))?;
    let [reference] = returned.reference_sources.as_slice() else {
        return unsupported("reference completion source roster changed");
    };
    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = source.source
    else {
        return unsupported("reference establishment requires retained ingress custody");
    };
    let parameter = machine
        .structural_parameters
        .get(parameter_index as usize)
        .ok_or(LoweringError::Unsupported(
            "reference ingress parameter is absent",
        ))?;
    if parameter.position as usize != position
        || parameter.access != CheckedStructuralAccess::MutableBorrow
        || parameter.access != source.access
        || parameter.type_identity != source.type_identity
        || !parameter.qualifications.is_empty()
        || !source.path.is_empty()
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
        || checked.normalized_type_identity(state.return_type).as_str() != binding.type_identity
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
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, caller.state)?;
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
    let (_, state) = crate::scalar_source_custody::authored_state(checked, caller.state)?;
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
        id,
        result: OperationResult::Structural(StructuralOperationResult {
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
