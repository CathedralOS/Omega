//! Reference carriers retain ingress provenance and the exact source loan end.
use super::super::StructuralArgument;
use super::admission::CallerView;
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
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, machine.state)?;
    if matches!(
        checked
            .statement_table
            .statements(state.statement_nodes)
            .get(binding.statement_index as usize),
        Some(StatementNode::LocalData(_))
    ) {
        return validate_local_establishment(checked, machine.state, binding, source);
    }
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
            && validation::reference_result_custody::tail_return_expression(&checked.typed, state)
                .and_then(|expression| {
                    validation::reference_result_custody::initializer_source(
                        &checked.typed,
                        state,
                        expression,
                        state.return_type,
                    )
                })
                .as_ref()
                == Some(source)
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
    } else if let Some(expected) = (|| {
        let expression =
            validation::reference_result_custody::tail_return_expression(&checked.typed, state)?;
        validation::reference_result_custody::borrowed_initializer_source(
            &checked.typed,
            state,
            expression,
            state.return_type,
        )
    })() {
        // `&receiver.field[i]` loan tails spell the same reconstruction the
        // record-completion roster uses: one projected source naming the
        // receiver position, the member and literal-index hops, and the
        // `Referent` segment — compared whole against the minted plan.
        expected == *source
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
        || checked.normalized_type_identity(state.return_type).as_str() != binding.type_identity
    {
        return unsupported("reference completion differs from its exact returned ingress");
    }
    Ok(())
}

/// A local loan replays the exact authored primitive source and retains the
/// full reference shell independently of the backing carrier.
pub(super) fn validate_local_establishment(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
    source: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let ordinal = result.statement_index as usize;
    let expected = validation::reference_result_custody::primitive_local_source(
        &checked.typed,
        authored,
        ordinal,
    )
    .ok_or(LoweringError::Unsupported(
        "primitive reference lost its authored source",
    ))?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(authored.statement_nodes)
        .get(ordinal)
    else {
        return unsupported("primitive reference has no local declaration");
    };
    if expected != *source
        || result.multiplicity != Multiplicity::Affine
        || checked
            .normalized_type_identity(local.type_reference)
            .as_str()
            != result.type_identity
    {
        return unsupported("primitive reference changed its source or declared carrier");
    }
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

/// Rejoin one composed state's `EstablishReference` to the authored
/// `&mut place` tail its return statement carries: the retained parameter,
/// the exact projected path and the declared referee must re-derive from
/// the typed source alone, the same replay `mutable_return_source` runs
/// for the caller-side loan.
pub(super) fn validate_composed_establishment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    result: &CheckedUnitStructuralResultBindingPlan,
    source: &checked_trees::CheckedUnitStructuralArgumentPlan,
    expression: checked_trees::expression::ExpressionHandle,
) -> Result<(), LoweringError> {
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state.state)?;
    if validation::reference_result_custody::shared_borrowed_parts(
        &checked.typed,
        authored.return_type,
    )
    .is_some()
    {
        if let Some(position) =
            validation::reference_result_custody::source_parameter(&checked.typed, authored)
        {
            // A shared `&'a V` return can forward the whole borrowed ingress
            // under the same reference convention `&mut` uses: the tail names
            // the parameter itself, so there is no projected path to rejoin —
            // only the parameter position, access, and type identities.
            let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
                source.source
            else {
                return unsupported("shared reference return requires retained ingress custody");
            };
            let parameter = state
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "shared reference ingress parameter is absent",
                ))?;
            let (referee, _) = validation::reference_result_custody::shared_borrowed_parts(
                &checked.typed,
                authored.return_type,
            )
            .ok_or(LoweringError::Unsupported(
                "shared reference return lost its declared referee",
            ))?;
            if parameter.position as usize != position
                || parameter.access != CheckedStructuralAccess::SharedBorrow
                || source.access != parameter.access
                || !source.path.is_empty()
                || checked.normalized_type_identity(referee).as_str() != source.type_identity
                || checked
                    .normalized_type_identity(authored.return_type)
                    .as_str()
                    != result.type_identity
                || result.multiplicity != Multiplicity::Affine
                || result.statement_index as usize + 1
                    != checked
                        .statement_table
                        .statements(authored.statement_nodes)
                        .len()
            {
                return unsupported("shared reference return differs from its exact ingress");
            }
            return Ok(());
        }
        // A projected shared tail — `&self.count`, `&receiver.field[i]` —
        // rejoins the same reconstruction the ordinary completion path runs:
        // one `Parameter` source, the member and literal-index hops, and the
        // trailing `Referent` segment, compared whole against the minted plan.
        let expected = validation::reference_result_custody::borrowed_initializer_source(
            &checked.typed,
            authored,
            expression,
            authored.return_type,
        )
        .ok_or(LoweringError::Unsupported(
            "shared reference return has no exact ingress source",
        ))?;
        if expected != *source
            || checked
                .normalized_type_identity(authored.return_type)
                .as_str()
                != result.type_identity
            || result.multiplicity != Multiplicity::Affine
            || result.statement_index as usize + 1
                != checked
                    .statement_table
                    .statements(authored.statement_nodes)
                    .len()
        {
            return unsupported("shared reference return differs from its exact ingress");
        }
        return Ok(());
    }
    let Some((position, path)) = validation::reference_result_custody::mutable_return_source(
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
    let parameter = state
        .structural_parameters
        .get(parameter_index as usize)
        .ok_or(LoweringError::Unsupported(
            "mutable reference ingress parameter is absent",
        ))?;
    let (referee, _) = validation::reference_result_custody::mutable_borrowed_parts(
        &checked.typed,
        authored.return_type,
    )
    .ok_or(LoweringError::Unsupported(
        "mutable reference return lost its declared referee",
    ))?;
    if parameter.position as usize != position
        || parameter.access != CheckedStructuralAccess::MutableBorrow
        || source.access != parameter.access
        || source.path != path
        || checked.normalized_type_identity(referee).as_str() != source.type_identity
        || checked
            .normalized_type_identity(authored.return_type)
            .as_str()
            != result.type_identity
        || result.multiplicity != Multiplicity::Affine
        || result.statement_index as usize + 1
            != checked
                .statement_table
                .statements(authored.statement_nodes)
                .len()
    {
        return unsupported("mutable reference return differs from its exact ingress");
    }
    Ok(())
}

/// A reference result's release in a composed state replays the weakening
/// the borrow checker recorded for the producing call's loan: the release
/// must pair with the unique earlier `StructuralCall` carrying that loan
/// under the same binding, at exactly the recorded boundary statement.
pub(super) fn validate_composed_release(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    release_index: usize,
    binding_ordinal: u32,
    loan: arena::Handle<checked_trees::BorrowLoanFact>,
    statement_index: u32,
) -> Result<(), LoweringError> {
    let mut producers = state
        .operations
        .iter()
        .enumerate()
        .filter(|(index, operation)| {
            *index < release_index
                && ordinary_structural_result(operation).is_some_and(|(result, custody)| {
                    result.binding_ordinal == binding_ordinal && custody.reference_loan == loan
                })
        });
    let Some((producer_index, producer)) = producers.next() else {
        return unsupported("reference release has no retained result loan");
    };
    if producers.next().is_some() {
        return unsupported("reference release pairs with more than one result loan");
    }
    super::structural_calls::validate_custody(checked, machine, state.state, producer)?;
    let boundary = validation::reference_result_custody::release_statement(
        &checked.facts,
        machine,
        state.state,
        loan,
    )
    .ok_or(LoweringError::Unsupported(
        "reference carrier has no exact weakening boundary",
    ))?;
    if boundary != statement_index || release_index <= producer_index {
        return unsupported("reference release differs from captured weakening");
    }
    // The boundary is before the first source operation at that statement,
    // never merely a matching coordinate emitted after a conflicting read.
    if state.operations[producer_index + 1..release_index]
        .iter()
        .any(|operation| {
            super::statement_custody::source_statement(operation)
                .is_some_and(|statement| statement >= boundary)
        })
    {
        return unsupported("reference loan release follows an already resumed source statement");
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
        super::structural_calls::validate_custody(
            checked,
            machine.machine,
            machine.state,
            operation,
        )?;
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
        let (_, authored) =
            crate::expression_preparation::source_custody::authored_state(checked, machine.state)?;
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
                super::statement_custody::source_statement(operation)
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
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller.state)?;
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
    let Some((producer, (result, custody))) = caller.operations.iter().find_map(|operation| {
        let produced = ordinary_structural_result(operation)?;
        (produced.0.binding_ordinal == binding_ordinal).then_some((operation, produced))
    }) else {
        return Ok(false);
    };
    if !custody.reference_loan.is_valid() {
        return Ok(false);
    }
    super::structural_calls::validate_custody(checked, caller.machine, caller.state, producer)?;
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller.state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("reference result consumer has no authored local");
    };
    let (referent, _) = validation::reference_result_custody::mutable_borrowed_parts(
        &checked.typed,
        local.type_reference,
    )
    .ok_or(LoweringError::Unsupported(
        "reference local lost its declared reference type",
    ))?;
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
    let (target, explicit_access) = match checked.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => (borrow.target, Some(borrow.access)),
        _ => (expression, None),
    };
    let (projected_path, root) =
        crate::expression_preparation::source_custody::structural_places::walk_exact_place(
            checked, machine, state, target, false,
        )?;
    if !projected_path.iter().all(|segment| {
        matches!(
            segment,
            CheckedUnitStructuralPathSegment::Field(_)
                | CheckedUnitStructuralPathSegment::FixedIndex(_)
        )
    }) || explicit_access
        .is_some_and(|access| access != language_semantics::ReferenceAccess::Mutable)
        || (!projected_path.is_empty() && explicit_access.is_none())
    {
        return unsupported("reference consumer is not an exact static mutable projection");
    }
    let selected = if projected_path.is_empty() {
        referent
    } else {
        validation::declared_place_type_raw(&checked.typed, machine, Some(state), target).ok_or(
            LoweringError::Unsupported("reference consumer projection lost its declared type"),
        )?
    };
    let mut expected_path = Vec::with_capacity(projected_path.len() + 1);
    expected_path.push(CheckedUnitStructuralPathSegment::Referent);
    expected_path.extend(projected_path);
    let boundary = validation::reference_result_custody::release_boundary(
        &checked.typed,
        &checked.facts,
        caller.machine,
        caller.state,
        custody.reference_loan,
    )
    .ok_or(LoweringError::Unsupported(
        "reference consumer has no weakening boundary",
    ))?;
    if argument.path != expected_path
        || argument.access != CheckedStructuralAccess::MutableBorrow
        || parameter.access != argument.access
        || argument.type_identity != parameter.type_identity
        || checked.normalized_type_identity(selected).as_str() != argument.type_identity
        || result.type_identity
            != checked
                .normalized_type_identity(local.type_reference)
                .as_str()
        || result.multiplicity != Multiplicity::Affine
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || parameter.multiplicity != Multiplicity::Unrestricted
        || coordinate.statement_index <= result.statement_index
        || coordinate.statement_index >= boundary
        || !matches!(checked.expression_table.expression(root), ExpressionNode::Name(path)
            if path.symbol == local.symbol && path.head_symbol == local.symbol
                && checked.expression_table.name_path_members(path.members).len() == 1)
    {
        return unsupported("reference consumer differs from its live captured referent");
    }
    if expected_path.len() > 1
        && !active_result_loan(checked, caller, coordinate, custody.reference_loan)
    {
        return unsupported("projected reference consumer has no active captured parent loan");
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
