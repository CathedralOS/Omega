//! Exact authored successor operands and exact-complement fallback pairing.
use super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::super::{
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, Multiplicity,
    terminal_scalar_type, unsupported,
};
use super::super::{CheckedTrees, LoweringError};
use super::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedStructuralControlSuccessorPlan, result_custody, subslices,
};
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::{
    StatementNode, TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

pub(in crate::unit::attached_unit::composed_control) fn successors(
    state: &CheckedComposedUnitControlStatePlan,
) -> Vec<&CheckedStructuralControlSuccessorPlan> {
    state.successors()
}

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    transition: &TableTransition,
    edge: &CheckedStructuralControlSuccessorPlan,
    ordinal: usize,
) -> Result<(), LoweringError> {
    validate_bindings(
        checked,
        plan,
        source,
        state,
        transition,
        edge,
        ordinal,
        &[],
        None,
    )?;
    validate_cleanup(checked, source, state, edge)
}

/// Case dispatch validates local-result cleanup separately from parameter cleanup.
///
/// `dispatched_parameter` is the authored position of the affine owned
/// parameter a closed-sum dispatch consumes, when its subject is one.
pub(super) fn validate_bindings(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    transition: &TableTransition,
    edge: &CheckedStructuralControlSuccessorPlan,
    ordinal: usize,
    payloads: &[checked_trees::CheckedClosedSumPayloadTransferPlan],
    dispatched_parameter: Option<u32>,
) -> Result<(), LoweringError> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = checked.statement_table.transition_target(transition.target)
    else {
        return unsupported("Unit graph edge has no named source target");
    };
    // An edge that spells a machine's own symbol enters that machine's
    // authored entry state; a fused roster holds foreign machines' entries
    // too, so resolve the named machine's entry rather than assuming the
    // dispatch machine's first position.
    let authored_target = match checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == path.symbol)
    {
        Some(machine) => {
            checked
                .machine_states(machine)
                .first()
                .ok_or(LoweringError::Unsupported(
                    "Unit graph edge machine has no authored entry",
                ))?
                .symbol
        }
        None => path.symbol,
    };
    if edge.statement_ordinal as usize != ordinal
        || edge.target_state != authored_target
        || transition.exit != TransitionExit::Ordinary
        || transition.continuation.is_valid()
    {
        return unsupported("Unit graph successor disagrees with source control");
    }
    // Every edge's owned-parameter discards are exactly those its checked
    // cleanup evidence names, less a closed-sum dispatch's own subject; an
    // edge without evidence discards none.
    //
    // The evidence counts a parameter as dying on the edge whenever the
    // edge's transition moves none of it. A case test moves nothing, and a
    // selected payload is a plain copy out of its parent, so the evidence
    // names the dispatched subject parameter on every case edge. Terminal
    // ownership accounts every owned obligation on an edge exactly once, and
    // on a closed-sum dispatch that death is the dispatch itself: the
    // StructuralCase terminator is the subject's explicit terminal
    // consumption. The edge naming the subject again as a no-code discard
    // would be a second disposition, so the subject's evidence row is required
    // and taken here as the dispatch's consumption. The opposite reading, that
    // the arm transfers the payload and discards the shell, would need the
    // dispatch to leave the subject live, which the StructuralCase terminator
    // does not. Guarded case tests outside a closed-sum dispatch keep the
    // subject among their ordinary edge discards.
    let cleanup = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_edge(
            super::owner(checked, state.state)?,
            state.state,
            edge.statement_ordinal,
        );
    let mut evidence = cleanup
        .map(|cleanup| cleanup.trivial_affine_discard_parameter_positions.to_vec())
        .unwrap_or_default();
    if let Some(subject) = dispatched_parameter
        && cleanup.is_some()
    {
        let Some(row) = evidence.iter().position(|position| *position == subject) else {
            return unsupported("Unit case edge cleanup evidence omits its dispatched subject");
        };
        evidence.remove(row);
    }
    if evidence != edge.trivial_affine_discard_parameter_positions {
        return unsupported("Unit graph successor discards disagree with cleanup evidence");
    }
    let target = plan
        .states
        .iter()
        .find(|state| state.state == edge.target_state)
        .ok_or(LoweringError::Unsupported(
            "Unit graph successor target is missing",
        ))?;
    let arguments = checked.statement_table.expression_handles(*arguments);
    // The authored operand list carries every position, including proof-only
    // erased formals the retained lanes strip.
    if arguments.len()
        != target
            .structural_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count()
            + target.scalar_parameters.len()
            + target.erased_scalar_parameters.len()
        || edge.transfers.len() != target.structural_parameters.len()
        || edge.scalar_arguments.len() + payloads.len() != target.scalar_parameters.len()
        || edge.erased_arguments.len() != target.erased_scalar_parameters.len()
    {
        return unsupported("Unit graph successor arity drifted");
    }
    let source_parameters = checked.state_parameters(source);
    // Authored operands omit an implicit `self` whether or not the target's
    // signature retained it: a graph-erased receiver still occupies its
    // authored formal position but no operand slot. Count authored `self`
    // formals, not signature rows, so both shapes map to the same operand.
    let target_formals =
        crate::expression_preparation::source_custody::authored_state(checked, edge.target_state)
            .map(|(_, state)| checked.state_parameters(state))?;
    let explicit_argument_position = |position: u32| {
        position as usize
            - target_formals
                .iter()
                .take(position as usize)
                .filter(|parameter| parameter.is_self)
                .count()
    };
    let validate_argument =
        |argument_position: u32, source_position: u32| {
            let argument_position = explicit_argument_position(argument_position);
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph source argument missing",
                ))?;
            let source = source_parameters.get(source_position as usize).ok_or(
                LoweringError::Unsupported("Unit graph source parameter missing"),
            )?;
            // Authored `self` resolves to the machine, while retained edge
            // custody names the state's receiver parameter. Only that exact
            // declared receiver may use the machine-root spelling.
            let source_symbol = if source.is_self {
                plan.machine
            } else {
                source.symbol
            };
            // `value as T` on a structural type ascribes the same parameter;
            // scalar (primitive-target) casts stay opaque to this binding.
            let mut argument = *expression;
            loop {
                match checked.expression_table.expression(argument) {
                    ExpressionNode::Cast(cast)
                        if checked.primitive_type_reference(cast.target_type).is_none() =>
                    {
                        argument = cast.value;
                    }
                    ExpressionNode::Borrow(borrow) => {
                        argument = borrow.target;
                    }
                    _ => break,
                }
            }
            match checked.expression_table.expression(argument) {
                ExpressionNode::Name(name)
                    if name.symbol == source_symbol
                        && name.head_symbol == source_symbol
                        && checked
                            .expression_table
                            .name_path_members(name.members)
                            .len()
                            == 1 =>
                {
                    Ok(())
                }
                _ => unsupported("Unit graph successor is not the retained parameter binding"),
            }
        };
    for (position, (target, transfer)) in target
        .structural_parameters
        .iter()
        .zip(&edge.transfers)
        .enumerate()
    {
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
            binding_ordinal,
        } = transfer.source
        {
            // Bindings establish availability; optional disposal metadata
            // only excludes a result that its automatic cleanup retires.
            let mut matching = state
                .operations
                .iter()
                .filter(|operation| {
                    !matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                            operand_source: Some(_),
                            ..
                        }
                    ) && !operation
                        .discardable_structural_result()
                        .is_some_and(|(_, discard)| discard)
                })
                .filter_map(|operation| {
                    operation.structural_result_binding().map(|result| {
                        (
                            result,
                            matches!(
                                operation,
                                CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
                            ),
                        )
                    })
                })
                .filter(|(result, _)| result.binding_ordinal == binding_ordinal);
            let (result, is_literal) = matching.next().ok_or(LoweringError::Unsupported(
                "Unit graph transferred result missing",
            ))?;
            // An owned local moves into the target; a view local lends its
            // shared view, which owns nothing and so leaves no partition. A
            // `&mut [T]` view local's affine binding is the exclusive loan
            // custody its `let` established; the target's mutable view
            // parameter continues that same loan.
            let expression = arguments
                .get(explicit_argument_position(target.position))
                .ok_or(LoweringError::Unsupported(
                    "Unit graph result argument missing",
                ))?;
            let borrowed_local = matches!(
                checked.expression_table.expression(*expression),
                ExpressionNode::Borrow(_)
            );
            let shared_view = target.access == checked_trees::CheckedStructuralAccess::SharedBorrow;
            let mutable_view = target.access
                == checked_trees::CheckedStructuralAccess::MutableBorrow
                && !borrowed_local
                && target.multiplicity == Multiplicity::Unrestricted
                && result.multiplicity == Multiplicity::Affine;
            let mutable_local = target.access
                == checked_trees::CheckedStructuralAccess::MutableBorrow
                && borrowed_local
                && plan.states.first().is_some_and(|entry| {
                    entry.state == state.state
                        && matches!(
                            entry.terminator,
                            CheckedComposedUnitControlTerminatorPlan::Jump { .. }
                        )
                })
                && target.multiplicity == Multiplicity::Unrestricted
                && matches!(
                    result.multiplicity,
                    Multiplicity::Affine | Multiplicity::Unrestricted
                );
            if matching.next().is_some()
                || (is_literal
                    && (!shared_view
                        || target.multiplicity != Multiplicity::Unrestricted
                        || !target.projected_qualifications.is_empty()))
                || result.statement_index >= edge.statement_ordinal
                || transfer.target_parameter_index as usize != position
                || target.is_self
                || !(target.access == checked_trees::CheckedStructuralAccess::Owned
                    || (shared_view && result.multiplicity == Multiplicity::Unrestricted)
                    || mutable_view
                    || mutable_local)
                || !target.qualifications.is_empty()
                || (!mutable_local && !mutable_view && target.multiplicity != result.multiplicity)
                || target.type_identity != result.type_identity
            {
                return unsupported("Unit graph transferred result custody drifted");
            }
            let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
                .statement_table
                .statements(source.statement_nodes)
                .get(result.statement_index as usize)
            else {
                return unsupported("Unit graph result source local missing");
            };
            let actual = if mutable_local {
                let ExpressionNode::Borrow(borrow) =
                    checked.expression_table.expression(*expression)
                else {
                    return unsupported("Unit graph local loan lost its mutable borrow");
                };
                if borrow.access != language_semantics::ReferenceAccess::Mutable
                    || (!local.is_mutable && local.initial_value.is_valid())
                    || !validation::has_plain_owned_contents_with_numeric_constraints(
                        &checked.typed,
                        local.type_reference,
                    )
                    || validation::type_graph_requires_nominal_drop(
                        &checked.typed,
                        local.type_reference,
                    )
                {
                    return unsupported("Unit graph local loan has unsupported backing custody");
                }
                borrow.target
            } else {
                if local.is_mutable {
                    return unsupported("Unit graph moved local is mutable");
                }
                *expression
            };
            if !matches!(checked.expression_table.expression(actual), ExpressionNode::Name(path) if path.symbol == local.symbol && path.head_symbol == local.symbol && checked.expression_table.name_path_members(path.members).len() == 1)
            {
                return unsupported("Unit graph result argument lost its actual local");
            }
            if shared_view || mutable_view {
                view_local_result(
                    checked,
                    source,
                    state,
                    local.symbol,
                    edge.statement_ordinal,
                    target.access,
                )?;
            } else {
                result_custody::validate_disposition(
                    checked,
                    super::owner(checked, state.state)?,
                    source,
                    local,
                    result,
                )?;
            }
            continue;
        }
        if let Some((root, _, collection, expression, _)) =
            subslices::transfer_range(&transfer.source)
        {
            // A view subslice narrows a whole view parameter or a view local
            // the state body established; either way the target receives a
            // fresh view with the requested access. A range over a
            // fixed-array field instead names the parameter owning the field
            // plus the checked field path, which emission views whole before
            // narrowing — the source's own type then names the record, not
            // the view. The range itself is replayed at its edge site when
            // the edge emits it.
            // Retained positions count the receiver formal; the authored
            // argument list does not.
            if arguments.get(explicit_argument_position(target.position)) != Some(&expression) {
                return unsupported("Unit graph subslice disagrees with its source argument");
            }
            if let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(expression)
            {
                let declared = validation::reference_shell(
                    &checked.typed,
                    target_formals[target.position as usize].type_reference,
                )
                .ok_or(LoweringError::Unsupported(
                    "Unit graph range target is not a reference",
                ))?;
                if borrow.access != declared.access {
                    return unsupported("Unit graph range changed its authored borrow access");
                }
            }
            let source_type = match root {
                checked_trees::CheckedStorageRoot::Parameter { index } => {
                    let source = state.structural_parameters.get(index as usize).ok_or(
                        LoweringError::Unsupported("Unit graph borrowed transfer source missing"),
                    )?;
                    if collection.is_empty() {
                        let access_matches = source.access == target.access
                            || (source.access
                                == checked_trees::CheckedStructuralAccess::MutableBorrow
                                && target.access
                                    == checked_trees::CheckedStructuralAccess::SharedBorrow);
                        if !access_matches || source.multiplicity != target.multiplicity {
                            return unsupported(
                                "Unit graph borrowed transfer type or order drifted",
                            );
                        }
                        Some(source.type_identity.clone())
                    } else {
                        // The field owner's root only lends the array: it
                        // must be readable and unqualified, and the target's
                        // view identity is its own peeled carrier identity —
                        // the authored path's agreement with the projection
                        // is replayed when the edge emits.
                        if !collection.iter().all(|segment| {
                            matches!(
                                segment,
                                checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                            )
                        }) || !matches!(
                            source.access,
                            checked_trees::CheckedStructuralAccess::SharedBorrow
                                | checked_trees::CheckedStructuralAccess::MutableBorrow
                        ) || (target.access
                            == checked_trees::CheckedStructuralAccess::MutableBorrow
                            && source.access
                                != checked_trees::CheckedStructuralAccess::MutableBorrow)
                            || !source.qualifications.is_empty()
                            || !source.projected_qualifications.is_empty()
                        {
                            return unsupported(
                                "Unit graph subslice field owner cannot lend the requested view",
                            );
                        }
                        let mut peeled = target_formals[target.position as usize].type_reference;
                        loop {
                            match checked.type_reference_table.type_reference(peeled) {
                                checked_trees::types::TypeReferenceNode::Constrained {
                                    base_type,
                                    ..
                                }
                                | checked_trees::types::TypeReferenceNode::Reference {
                                    referee: base_type,
                                    ..
                                } => peeled = *base_type,
                                _ => break,
                            }
                        }
                        Some(checked.normalized_type_identity(peeled).into_string())
                    }
                }
                checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                    if !collection.is_empty() {
                        return unsupported("Unit graph subslice field range lost its owner");
                    }
                    Some(
                        view_local_result(
                            checked,
                            source,
                            state,
                            symbol,
                            edge.statement_ordinal,
                            target.access,
                        )?
                        .to_string(),
                    )
                }
                checked_trees::CheckedStorageRoot::OwnedLocal { .. } => {
                    return unsupported(
                        "Unit graph borrowed transfer source is an owned local, not a view",
                    );
                }
            };
            if transfer.target_parameter_index as usize != position
                || target.is_self
                || !matches!(
                    target.access,
                    checked_trees::CheckedStructuralAccess::SharedBorrow
                        | checked_trees::CheckedStructuralAccess::MutableBorrow
                )
                || target.multiplicity != Multiplicity::Unrestricted
                || source_type.as_deref() != Some(target.type_identity.as_str())
            {
                return unsupported("Unit graph borrowed transfer type or order drifted");
            }
            continue;
        }
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue {
            operands,
            establishment,
        } = &transfer.source
        {
            // The authored operand is the construction, call, or copied place:
            // it is the one value the statement registers as rooted at that
            // expression and its declared carrier is the target's. A copied
            // place must replay its exact source/path and unrestricted leaf below; this
            // is fresh destination storage, not a transfer of source custody.
            // The edge's
            // establishment is an ordinary one at the transition's own
            // coordinate, producing the target's carrier under its exact
            // owned or borrowed custody, and replays as a `let` initializer's does,
            // embedded calls included.
            let result =
                establishment
                    .structural_result_binding()
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph established argument has no result",
                    ))?;
            let argument_position = explicit_argument_position(target.position);
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph established argument missing",
                ))?;
            let Some(root) = checked.facts.values.structural_values.root_for_expression(
                source.symbol,
                edge.statement_ordinal,
                *expression,
            ) else {
                return unsupported("Unit graph established argument lost its registration");
            };
            // A borrowed formal receives descriptor access, not ownership of
            // the descriptor carrier. Its mutable producer retains affine
            // disposal debt while the formal itself is unrestricted. The
            // common construction replay below independently checks this
            // producer against the exact authored transition operand/formal.
            let custody_matches = match target.access {
                checked_trees::CheckedStructuralAccess::Owned => {
                    result.multiplicity == target.multiplicity
                        && checked
                            .normalized_type_identity(root.type_reference)
                            .as_str()
                            == target.type_identity
                }
                checked_trees::CheckedStructuralAccess::SharedBorrow
                | checked_trees::CheckedStructuralAccess::MutableBorrow => {
                    validation::slice_view_shell(&checked.typed, root.type_reference, &[])
                        .is_some_and(|view| {
                            let access: checked_trees::CheckedStructuralAccess = view.access.into();
                            let multiplicity = if access
                                == checked_trees::CheckedStructuralAccess::MutableBorrow
                            {
                                Multiplicity::Affine
                            } else {
                                Multiplicity::Unrestricted
                            };
                            access == target.access
                                && target.multiplicity == Multiplicity::Unrestricted
                                && result.multiplicity == multiplicity
                                && crate::unit::attached_unit::structural_carrier_type(
                                    checked, view.slice,
                                )
                                .is_ok_and(|carrier| {
                                    checked.normalized_type_identity(carrier).as_str()
                                        == target.type_identity
                                })
                        })
                }
                _ => false,
            };
            if root.machine != plan.machine
                || result.statement_index != edge.statement_ordinal
                || result.type_identity != target.type_identity
                || transfer.target_parameter_index as usize != position
                || target.is_self
                || !custody_matches
            {
                return unsupported("Unit graph established argument custody drifted");
            }
            // Preparation is owned by the exact transition and each
            // constructor rejoins its consuming call/formal independently.
            // This cannot import a predecessor's or a sibling edge's work.
            for operand in operands.iter() {
                let binding =
                    operand
                        .structural_result_binding()
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph operand has no result binding",
                        ))?;
                if binding.statement_index != edge.statement_ordinal {
                    return unsupported("Unit graph operand left its selected transition");
                }
                match operand {
                    checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                        operand_source: Some(_),
                        ..
                    } => {
                        crate::expression_preparation::source_custody::structural::validate(
                            checked,
                            plan.machine,
                            state.state,
                            operand,
                        )?;
                    }
                    checked_trees::CheckedUnitEffectOperationPlan::EstablishScalarArray {
                        source,
                        result,
                        elements,
                    } => {
                        super::super::super::scalar_arrays::validate(
                            checked,
                            plan.machine,
                            state.state,
                            *source,
                            result,
                            elements,
                        )?;
                    }
                    checked_trees::CheckedUnitEffectOperationPlan::Call(_) => {
                        crate::emission::call_source_custody::validate_operation(
                            checked,
                            plan.machine,
                            state.state,
                            operand,
                            &state.structural_parameters,
                        )?;
                    }
                    _ => return unsupported("Unit graph operand is not call preparation"),
                }
            }
            // The operation's common source replay owns producer admission,
            // including primitive arrays and composed values. Keeping another
            // value-kind catalog on state edges would give the same operation
            // different permissions solely because it feeds a successor.
            match establishment.as_ref() {
                checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    value,
                    operand_source: None,
                    discard_result_on_return: false,
                    ..
                } if *value == root.root => {
                    crate::expression_preparation::source_custody::structural::validate(
                        checked,
                        plan.machine,
                        state.state,
                        establishment,
                    )?;
                }
                checked_trees::CheckedUnitEffectOperationPlan::Call(call) => {
                    super::super::super::argument_schedule::validate_operand_preparation(
                        checked,
                        plan.machine,
                        state.state,
                        operands,
                        establishment,
                    )?;
                    if call.discard_result_on_return() != Some(false) {
                        return unsupported(
                            "Unit graph call argument is discarded before transfer",
                        );
                    }
                    crate::expression_preparation::source_custody::structural::validate_call_value(
                        checked,
                        plan.machine,
                        state.state,
                        *expression,
                        establishment,
                        &state.structural_parameters,
                    )?;
                }
                _ => return unsupported("Unit graph argument is not a fresh value producer"),
            }
            continue;
        }
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
            subject,
            carrier_identity,
        } = &transfer.source
        {
            // Rejoin the argument's declared reference correspondence, not
            // its spelling: a shared field loan needs no explicit `&` token.
            // Its access matches the loan's and the target's, the projected
            // identity is the target's, the path carries only the segments
            // the admit side admits, and the subject roots at a parameter or
            // a rejoinable result binding.
            let argument_position = explicit_argument_position(target.position);
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph loan argument missing",
                ))?;
            let declared_target =
                target_formals
                    .get(target.position as usize)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph loan target declaration missing",
                    ))?;
            let access_permitted = validation::checked_argument_matches_type_reference(
                &checked.typed,
                *expression,
                declared_target.type_reference,
            ) && matches!(
                subject.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
                    | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
            );
            let source_permitted = match subject.source {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                } => state
                    .structural_parameters
                    .get(parameter_index as usize)
                    .is_some_and(|parameter| {
                        use checked_trees::CheckedStructuralAccess as Access;
                        matches!(
                            (parameter.access, subject.access),
                            (
                                Access::Owned | Access::MutableBorrow,
                                Access::SharedBorrow
                                    | Access::MutableBorrow
                                    | Access::WriteOnlyBorrow
                            ) | (Access::SharedBorrow, Access::SharedBorrow)
                                | (Access::WriteOnlyBorrow, Access::WriteOnlyBorrow)
                        )
                    }),
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    ..
                } => true,
                _ => false,
            };
            if transfer.target_parameter_index as usize != position
                || !access_permitted
                || !source_permitted
                || subject.access != target.access
                || subject.type_identity != target.type_identity
                || target.multiplicity != Multiplicity::Unrestricted
                || target.is_self
                || carrier_identity.is_empty() != subject.path.is_empty()
                || !subject.path.iter().all(|segment| {
                    matches!(
                        segment,
                        checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                            | checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                            | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                    )
                })
            {
                return unsupported("Unit graph loan argument custody drifted");
            }
            validate_projected_loan_source(
                checked,
                plan.machine,
                source,
                state,
                edge.statement_ordinal,
                *expression,
                subject,
            )?;
            continue;
        }
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
            subject,
            case_identity,
            field_identity,
            path: payload_path,
        } = &transfer.source
        {
            // The edge's case test selects this case; the declared payload
            // field feeds the target's owned custody with its own type and
            // multiplicity, and the authored operand is the case-qualified
            // member of that field on the very subject the guard tests.
            if transfer.target_parameter_index as usize != position || !payload_path.is_empty() {
                return unsupported("Unit graph case-payload transfer drifted");
            }
            let TransitionGuardNode::When(guard) = transition.guard else {
                return unsupported("Unit graph case-payload edge lacks a case test");
            };
            let Some((tested, variant_symbol)) = super::cases::case_test(checked, guard) else {
                return unsupported("Unit graph case-payload guard lost its case test");
            };
            let Some((_, variant)) = checked.data_definitions().iter().find_map(|data| {
                checked.data_members(data).iter().find_map(|member| {
                    let checked_trees::data::DataMember::Variant(variant) = member else {
                        return None;
                    };
                    (variant.symbol == variant_symbol).then_some((data, variant))
                })
            }) else {
                return unsupported("Unit graph case-payload case is missing");
            };
            if variant.path_identity() != *case_identity {
                return unsupported("Unit graph case-payload case identity drifted");
            }
            let Some(field) = checked
                .data_payload_fields(variant)
                .iter()
                .find(|field| field.path_identity() == *field_identity)
            else {
                return unsupported("Unit graph case-payload field is missing");
            };
            // An `Owned` target copies the member's whole identity; a
            // `SharedBorrow` target re-seats a member that is itself a
            // shared borrow, comparing the referee's normalized identity —
            // a view parameter declares its referent's identity.
            let mut referee_cursor = field.type_reference;
            let shared_referee = loop {
                match checked.type_reference_table.type_reference(referee_cursor) {
                    checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                        referee_cursor = *base_type
                    }
                    checked_trees::types::TypeReferenceNode::Reference {
                        access: language_semantics::ReferenceAccess::Shared,
                        referee,
                        ..
                    } => break Some(*referee),
                    _ => break None,
                }
            };
            let normalized = |reference| checked.normalized_type_identity(reference).into_string();
            let admitted = if target.access == checked_trees::CheckedStructuralAccess::Owned {
                normalized(field.type_reference) == target.type_identity
            } else {
                target.access == checked_trees::CheckedStructuralAccess::SharedBorrow
                    && shared_referee
                        .is_some_and(|referee| normalized(referee) == target.type_identity)
            };
            if !admitted || checked.type_multiplicity(field.type_reference) != target.multiplicity {
                return unsupported("Unit graph case-payload custody drifted");
            }
            if subject.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
                return unsupported("Unit graph case-payload subject custody drifted");
            }
            // Resolve the same declared root whether the case arrived as a
            // parameter or was returned by an earlier operation. Result
            // identity alone is not evidence: rejoin its authored local and
            // establishment custody before reading its payload.
            let (root_symbol, mut reference, root_identity) = match subject.source {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                } => {
                    let retained = state
                        .structural_parameters
                        .get(parameter_index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph case-payload subject missing",
                        ))?;
                    let parameter = source_parameters.get(retained.position as usize).ok_or(
                        LoweringError::Unsupported("Unit graph case-payload parameter missing"),
                    )?;
                    (
                        if parameter.is_self {
                            plan.machine
                        } else {
                            parameter.symbol
                        },
                        parameter.type_reference,
                        retained.type_identity.as_str(),
                    )
                }
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                } => {
                    let mut results = state
                        .operations
                        .iter()
                        .filter_map(result_custody::result)
                        .filter(|result| result.binding_ordinal == binding_ordinal);
                    let result = results.next().ok_or(LoweringError::Unsupported(
                        "Unit graph case-payload result missing",
                    ))?;
                    if results.next().is_some() || result.statement_index >= edge.statement_ordinal
                    {
                        return unsupported("Unit graph case-payload result establishment drifted");
                    }
                    let Some(StatementNode::LocalData(local)) = checked
                        .statement_table
                        .statements(source.statement_nodes)
                        .get(result.statement_index as usize)
                    else {
                        return unsupported("Unit graph case-payload result local missing");
                    };
                    if local.is_mutable {
                        return unsupported("Unit graph case-payload result local is mutable");
                    }
                    result_custody::validate_disposition(
                        checked,
                        plan.machine,
                        source,
                        local,
                        result,
                    )?;
                    (
                        local.symbol,
                        local.type_reference,
                        result.type_identity.as_str(),
                    )
                }
                _ => return unsupported("Unit graph case-payload subject unsupported"),
            };
            let mut tested_root = tested;
            let mut tested_fields = Vec::new();
            while let ExpressionNode::Member(member) =
                checked.expression_table.expression(tested_root)
            {
                if member.case_variant.is_some() {
                    return unsupported(
                        "Unit graph case-payload subject has an unproven case projection",
                    );
                }
                tested_fields.push(member);
                tested_root = member.receiver;
            }
            if !matches!(checked.expression_table.expression(tested_root), ExpressionNode::Name(name)
                if name.symbol == root_symbol && name.head_symbol == root_symbol && name.members.count() == 1)
                || tested_fields.len() != subject.path.len()
            {
                return unsupported("Unit graph case-payload subject differs from its guard");
            }
            let Some(mut owner) = nominal_owner(checked, reference) else {
                return unsupported("Unit graph case-payload subject is not nominal");
            };
            for (position, (segment, tested_field)) in subject
                .path
                .iter()
                .zip(tested_fields.iter().rev())
                .enumerate()
            {
                let checked_trees::CheckedUnitStructuralPathSegment::Field(identity) = segment
                else {
                    return unsupported("Unit graph case-payload path is not a member walk");
                };
                let Some(member) = checked.data_members(owner).iter().find_map(|member| {
                    let checked_trees::data::DataMember::Field(field) = member else {
                        return None;
                    };
                    // A typed `self` field is a receiver-view symbol; the
                    // attached owner's field is resolved by its member name.
                    ((field.symbol == tested_field.member_symbol
                        || (position == 0
                            && root_symbol == plan.machine
                            && field.name == tested_field.member))
                        && field
                            .identity
                            .map(|position| format!("#{position}"))
                            .unwrap_or_else(|| field.name.as_str().to_owned())
                            == *identity)
                        .then_some(field)
                }) else {
                    return unsupported("Unit graph case-payload path field is missing");
                };
                reference = member.type_reference;
                let Some(next) = nominal_owner(checked, reference) else {
                    return unsupported("Unit graph case-payload path is not nominal");
                };
                owner = next;
            }
            // The recorded identity uses the checked parameter's canonical
            // spelling for a bare subject — attached `self` names the
            // machine's nominal type in the typed table — and the peeled
            // leaf's normalized identity under a member projection.
            let replayed_identity = if subject.path.is_empty() {
                root_identity.to_owned()
            } else {
                while let checked_trees::types::TypeReferenceNode::Constrained {
                    base_type, ..
                }
                | checked_trees::types::TypeReferenceNode::Reference {
                    referee: base_type,
                    ..
                } = checked.type_reference_table.type_reference(reference)
                {
                    reference = *base_type;
                }
                checked.normalized_type_identity(reference).into_string()
            };
            if replayed_identity != subject.type_identity
                || !checked.data_members(owner).iter().any(|member| {
                    matches!(
                        member,
                        checked_trees::data::DataMember::Variant(variant)
                            if variant.symbol == variant_symbol
                    )
                })
            {
                return unsupported("Unit graph case-payload subject is not the tested sum");
            }
            let argument_position = explicit_argument_position(target.position);
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph case-payload argument missing",
                ))?;
            match checked.expression_table.expression(*expression) {
                ExpressionNode::Member(member)
                    if member.member == field.name
                        && member.case_variant.as_ref().map(|v| v.as_str())
                            == Some(variant.name.as_str())
                        && checked
                            .expression_table
                            .expressions_structurally_equal(tested, member.receiver) => {}
                _ => return unsupported("Unit graph case-payload argument lost its binding"),
            }
            continue;
        }
        if matches!(
            transfer.source,
            checked_trees::CheckedStructuralControlTransferSourcePlan::MachineReceiver
        ) {
            // A source state that never declared `self` forwards the
            // machine's ambient receiver: the target's `self` binds the
            // machine's entry receiver under the same access narrowing a
            // carried parameter would take.
            let Some(receiver) = plan.states[0]
                .structural_parameters
                .iter()
                .find(|parameter| parameter.is_self)
            else {
                return unsupported("Unit graph machine receiver is missing");
            };
            if !target.is_self
                || transfer.target_parameter_index as usize != position
                || !super::parameters::receiver_transfer_matches(receiver, target)
            {
                return unsupported("Unit graph machine receiver custody drifted");
            }
            continue;
        }
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedCopy {
            subject,
        } = &transfer.source
        {
            // The authored operand is the projected place itself —
            // `param.field` — reading a copyable subtree out of a live
            // parameter root. The subject names the root and the member
            // path; the authored operand must walk exactly that path
            // below that root, the leaf's identity and `Unrestricted`
            // declaration must match the `Owned` target's, and every
            // segment must be a field or fixed index.
            let argument_position = explicit_argument_position(target.position);
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph projected argument missing",
                ))?;
            // An authored `self.field` names the receiver's own symbol, while
            // the implicit receiver roots at the machine; either spelling
            // names the one retained receiver parameter.
            let (root_symbol, receiver_root, mut reference) = match subject.source {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                } => {
                    let retained = state
                        .structural_parameters
                        .get(parameter_index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph projected subject missing",
                        ))?;
                    if retained.access == checked_trees::CheckedStructuralAccess::WriteOnlyBorrow {
                        return unsupported("Unit graph projected subject is write-only");
                    }
                    let parameter = source_parameters.get(retained.position as usize).ok_or(
                        LoweringError::Unsupported("Unit graph projected parameter missing"),
                    )?;
                    (
                        parameter.symbol,
                        parameter.is_self.then_some(plan.machine),
                        parameter.type_reference,
                    )
                }
                _ => return unsupported("Unit graph projected subject unsupported"),
            };
            let mut tested_root = *expression;
            let mut members = Vec::new();
            while let ExpressionNode::Member(member) =
                checked.expression_table.expression(tested_root)
            {
                if member.case_variant.is_some() {
                    return unsupported("Unit graph projected argument has a case projection");
                }
                members.push(member);
                tested_root = member.receiver;
            }
            members.reverse();
            // Replay the member walk from the root's own declared type: each
            // segment names one field of the data its receiver has, by symbol
            // or — for a typed `self` receiver view — by its member name.
            let mut path_matches = members.len() == subject.path.len();
            for (segment, member) in subject.path.iter().zip(&members) {
                let checked_trees::CheckedUnitStructuralPathSegment::Field(identity) = segment
                else {
                    path_matches = false;
                    break;
                };
                let field = nominal_owner(checked, reference).and_then(|owner| {
                    checked.data_members(owner).iter().find_map(|candidate| {
                        let checked_trees::data::DataMember::Field(field) = candidate else {
                            return None;
                        };
                        ((field.symbol == member.member_symbol || field.name == member.member)
                            && field.path_identity() == *identity)
                            .then_some(field)
                    })
                });
                let Some(field) = field else {
                    path_matches = false;
                    break;
                };
                reference = field.type_reference;
            }
            // The copied leaf is the last field's declared type: an
            // unrestricted value matching the target's declared identity, with
            // represented owned-field predicates or shared views retaining
            // their source loans. A shared-view leaf copies only its
            // descriptor — the view's loan keeps its lineage through the
            // projection's root — so it compares its referee's identity to the
            // target's declared identity the way the subject's mint did. The
            // signature's structural identities are storage identities: the
            // domain shell peels before the compare.
            let normalized = |reference| {
                let reference = crate::unit::attached_unit::parameters::structural_carrier_type(
                    checked, reference,
                )
                .unwrap_or(reference);
                checked.normalized_type_identity(reference).into_string()
            };
            let leaf_copyable = path_matches
                && checked.type_multiplicity(reference) == Multiplicity::Unrestricted
                && match subject.access {
                    checked_trees::CheckedStructuralAccess::Owned => {
                        normalized(reference) == target.type_identity
                            && (validation::has_plain_owned_contents_with_represented_constraints(
                                &checked.typed,
                                reference,
                            ) || validation::has_owned_or_shared_view_fields(
                                &checked.typed,
                                reference,
                            ))
                    }
                    checked_trees::CheckedStructuralAccess::SharedBorrow => {
                        let mut cursor = reference;
                        loop {
                            match checked.type_reference_table.type_reference(cursor) {
                                checked_trees::types::TypeReferenceNode::Constrained {
                                    base_type,
                                    ..
                                } => cursor = *base_type,
                                checked_trees::types::TypeReferenceNode::Reference {
                                    access: language_semantics::ReferenceAccess::Shared,
                                    referee,
                                    ..
                                } => break normalized(*referee) == target.type_identity,
                                _ => break false,
                            }
                        }
                    }
                    _ => false,
                };
            let name_ok = matches!(
                checked.expression_table.expression(tested_root),
                ExpressionNode::Name(name)
                    if (name.symbol == root_symbol || Some(name.symbol) == receiver_root)
                        && name.head_symbol == name.symbol
                        && name.members.count() == 1
            );
            let access_ok = subject.access == target.access
                && matches!(
                    subject.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                );
            if transfer.target_parameter_index as usize != position
                || !name_ok
                || !path_matches
                || !leaf_copyable
                || !access_ok
                || subject.type_identity != target.type_identity
                || target.multiplicity != Multiplicity::Unrestricted
                || target.is_self
                || subject.path.is_empty()
            {
                return unsupported("Unit graph projected argument custody drifted");
            }
            continue;
        }
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral {
            bytes,
        } = &transfer.source
        {
            // A literal argument has no custody root: the edge establishes
            // the authored octets into immutable storage the borrowed
            // byte-view target reads. The authored operand must be the
            // string literal those bytes came from, and the target a whole
            // shared view of the literal's type identity.
            let argument_position = explicit_argument_position(target.position);
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph literal argument missing",
                ))?;
            let ExpressionNode::String(actual) = checked.expression_table.expression(*expression)
            else {
                return unsupported("Unit graph literal argument lost its bytes");
            };
            if transfer.target_parameter_index as usize != position
                || actual.as_ref() != bytes.as_slice()
                || target.access != checked_trees::CheckedStructuralAccess::SharedBorrow
                || target.multiplicity != Multiplicity::Unrestricted
                || target.is_self
            {
                return unsupported("Unit graph literal argument custody drifted");
            }
            continue;
        }
        let source_index = match transfer.source {
            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
                ..
            } => return unsupported("Unit graph result was not independently rejoined"),
            checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => index,
            checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
                ..
            }
            | checked_trees::CheckedStructuralControlTransferSourcePlan::ElementViewSubslice {
                ..
            } => return unsupported("Unit graph subslice was not independently rejoined"),
            checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                ..
            } => {
                return unsupported(
                    "Unit graph established argument was not independently rejoined",
                );
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { .. } => {
                return unsupported("Unit graph loan argument was not independently rejoined");
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload { .. } => {
                return unsupported("Unit graph case-payload was not independently rejoined");
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral {
                ..
            } => {
                return unsupported("Unit graph literal argument was not independently rejoined");
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::MachineReceiver => {
                return unsupported("Unit graph machine receiver was not independently rejoined");
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedCopy { .. } => {
                return unsupported("Unit graph projected argument was not independently rejoined");
            }
        };
        let source = state
            .structural_parameters
            .get(source_index as usize)
            .ok_or(LoweringError::Unsupported(
                "Unit graph borrowed transfer source missing",
            ))?;
        let access_matches = source.access == target.access
            || (source.access == checked_trees::CheckedStructuralAccess::MutableBorrow
                && target.access == checked_trees::CheckedStructuralAccess::SharedBorrow);
        if transfer.target_parameter_index as usize != position
            || source.type_identity != target.type_identity
            || (!target.is_self && !access_matches)
            || source.multiplicity != target.multiplicity
        {
            return unsupported("Unit graph borrowed transfer type or order drifted");
        }
        match transfer.source {
            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
                ..
            } => return unsupported("Unit graph result was not independently rejoined"),
            checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { .. } => {
                if target.is_self {
                    if !super::parameters::receiver_transfer_matches(source, target) {
                        return unsupported("Unit graph persistent receiver transfer drifted");
                    }
                } else {
                    validate_argument(target.position, source.position)?;
                }
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
                ..
            }
            | checked_trees::CheckedStructuralControlTransferSourcePlan::ElementViewSubslice {
                ..
            } => return unsupported("Unit graph subslice was not independently rejoined"),
            checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                ..
            } => {
                return unsupported(
                    "Unit graph established argument was not independently rejoined",
                );
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan { .. } => {
                return unsupported("Unit graph loan argument was not independently rejoined");
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload { .. } => {
                return unsupported("Unit graph case-payload was not independently rejoined");
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral {
                ..
            } => {
                return unsupported("Unit graph literal argument was not independently rejoined");
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::MachineReceiver => {
                return unsupported("Unit graph machine receiver was not independently rejoined");
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedCopy { .. } => {
                return unsupported("Unit graph projected argument was not independently rejoined");
            }
        }
    }
    for ((position, target), transfer) in target
        .scalar_parameters
        .iter()
        .enumerate()
        .filter(|(position, _)| {
            !payloads
                .iter()
                .any(|payload| payload.target_scalar_parameter_index as usize == *position)
        })
        .zip(&edge.scalar_arguments)
    {
        if transfer.target_scalar_parameter_index as usize != position
            || transfer.argument_ordinal != target.source_position
            || transfer.primitive_type != target.primitive_type
        {
            return unsupported("Unit graph scalar transfer type or order drifted");
        }
        match transfer.source {
            checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index } => {
                let source = state.scalar_parameters.get(index as usize).ok_or(
                    LoweringError::Unsupported("Unit graph scalar transfer source missing"),
                )?;
                if source.primitive_type != target.primitive_type {
                    return unsupported("Unit graph scalar transfer source type drifted");
                }
                validate_argument(target.source_position, source.source_position)?;
            }
            checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression => {
                super::scalars::successor_value(checked, state, edge, transfer)?;
            }
        }
    }
    // Proof-only lanes carry the same authored expression custody as the
    // retained arguments; erased formals never bind a runtime parameter.
    for ((position, target), transfer) in target
        .erased_scalar_parameters
        .iter()
        .enumerate()
        .zip(&edge.erased_arguments)
    {
        if transfer.target_scalar_parameter_index as usize != position
            || transfer.argument_ordinal != target.source_position
            || transfer.primitive_type != target.primitive_type
            || !matches!(
                transfer.source,
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
            )
        {
            return unsupported("Unit graph erased transfer type or order drifted");
        }
        let (binding, _) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(
                state.state,
                edge.statement_ordinal,
                CheckedScalarExpressionRole::TransitionArgument {
                    argument_ordinal: transfer.argument_ordinal,
                },
            )
            .ok_or(LoweringError::Unsupported(
                "Unit graph erased successor has no checked source expression",
            ))?;
        crate::expression_preparation::source_custody::validate_pure(
            checked,
            binding,
            terminal_scalar_type(target.primitive_type)?,
        )?;
    }
    Ok(())
}

/// The type identity of the view local `symbol` that the state body
/// established before `statement`, lent at `access`: the unique view result
/// an `as_slice`/`as_mut_slice` loan or a subslice binding published for that
/// `let`. A shared view's binding is an unrestricted observation. A mutable
/// view's binding is the affine custody of the exclusive loan its `let` took,
/// which only a local declared `&mut [T]` holds; a shared view local never
/// lends mutably.
fn view_local_result<'a>(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    state: &'a CheckedComposedUnitControlStatePlan,
    symbol: symbols::SymbolHandle,
    statement: u32,
    access: checked_trees::CheckedStructuralAccess,
) -> Result<&'a String, LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    let mut matching = state.operations.iter().filter_map(|operation| {
        let result = match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. } => result,
            _ => return None,
        };
        match statements.get(result.statement_index as usize) {
            Some(checked_trees::statement::StatementNode::LocalData(local))
                if local.symbol == symbol && !local.is_mutable =>
            {
                Some((result, local, operation))
            }
            _ => None,
        }
    });
    let (result, local, operation) = matching.next().ok_or(LoweringError::Unsupported(
        "Unit graph view local has no establishment",
    ))?;
    let lent = match access {
        checked_trees::CheckedStructuralAccess::SharedBorrow => {
            result.multiplicity == Multiplicity::Unrestricted
        }
        checked_trees::CheckedStructuralAccess::MutableBorrow => {
            result.multiplicity == Multiplicity::Affine
                && matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                )
                && validation::slice_view_shell(&checked.typed, local.type_reference, &[])
                    .is_some_and(|view| view.access == language_semantics::ReferenceAccess::Mutable)
        }
        _ => false,
    };
    if matching.next().is_some() || result.statement_index >= statement || !lent {
        return unsupported("Unit graph view local establishment drifted");
    }
    Ok(&result.type_identity)
}

fn validate_cleanup(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    edge: &CheckedStructuralControlSuccessorPlan,
) -> Result<(), LoweringError> {
    let owner = super::owner(checked, state.state)?;
    result_custody::validate_disposition_roster(checked, owner, source, state)?;
    result_custody::local_discards(checked, owner, source, state, Some(edge))?;
    let cleanup = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_edge(owner, state.state, edge.statement_ordinal)
        .ok_or(LoweringError::Unsupported(
            "Unit graph edge cleanup evidence missing",
        ))?;
    if cleanup.target_state != edge.target_state
        || cleanup.trivial_affine_discard_parameter_positions
            != edge.trivial_affine_discard_parameter_positions
    {
        return unsupported("Unit graph edge cleanup evidence disagrees");
    }
    Ok(())
}

/// Only stable place reads may be compared as repeated case observations.
pub(super) fn case_subject_is_place(
    checked: &CheckedTrees,
    mut expression: checked_trees::expression::ExpressionHandle,
) -> bool {
    loop {
        match checked.expression_table.expression(expression) {
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                expression = member.receiver;
            }
            ExpressionNode::Name(name) => return name.symbol.is_valid(),
            // Equal call syntax does not establish equal observations; calls
            // may have effects or produce a different case on each evaluation.
            _ => return false,
        }
    }
}

/// Rejoin exact case coverage or the ordinary pure guard-complement proof.
pub(super) fn validate_fallback(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    guard_ordinal: usize,
    when_false: &TableTransition,
) -> Result<(), LoweringError> {
    if when_false.guard == TransitionGuardNode::Always {
        return Ok(());
    }
    let guard_ordinal = u32::try_from(guard_ordinal)
        .map_err(|_| LoweringError::Unsupported("Unit graph ordinal overflow"))?;
    // Case tests on a local result are computations, not pure guard rows.
    // Reconstruct their complement from the authored pair and the closed sum
    // declaration rather than trusting a retained coverage flag. The ordinary
    // guard rejoin still authenticates the value actually emitted.
    let (_, source) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    if let Some(StatementNode::Transition(when_true)) = checked.statement_table
        .statements(source.statement_nodes).get(guard_ordinal as usize)
        && let (TransitionGuardNode::When(first), TransitionGuardNode::When(second)) =
            (when_true.guard, when_false.guard)
        && let (Some((first_subject, first_case)), Some((second_subject, second_case))) =
            (super::cases::case_test(checked, first), super::cases::case_test(checked, second))
        && first_case != second_case
        && case_subject_is_place(checked, first_subject)
        && checked.expression_table.expressions_structurally_equal(first_subject, second_subject)
        && checked.data_definitions().iter().any(|data| {
            let members = checked.data_members(data);
            matches!(members, [checked_trees::data::DataMember::Variant(first), checked_trees::data::DataMember::Variant(second)]
                if (first.symbol == first_case && second.symbol == second_case)
                    || (first.symbol == second_case && second.symbol == first_case))
        })
    {
        return Ok(());
    }
    if crate::expression_preparation::source_custody::guard_complement::complementary(
        checked,
        state,
        guard_ordinal,
    )? {
        return Ok(());
    }
    // A retained guarded pair with no authored fallback is covering by the
    // shared roster's own proof; the second guard is the vacuous else.
    let covering_pair = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .iter()
        .filter(|tail| tail.state == state)
        .any(|retained| {
            retained.fallback.is_none()
                && checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_exits
                    .span(retained.arms)
                    .is_some_and(|exits| {
                        exits.len() == 2
                            && exits[0].guard_statement_ordinal == guard_ordinal
                            && exits[1].guard_statement_ordinal == guard_ordinal + 1
                    })
        });
    if covering_pair {
        Ok(())
    } else {
        unsupported("Unit graph fallback is not the exact complement of its guard")
    }
}

/// Reconstruct the remaining whole-parameter drops at an ordinary return.
/// Call consumption and return disposal are separate uses of the same source root.
pub(super) fn return_discards(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<Vec<usize>, LoweringError> {
    use language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
        PermissionProvenance,
    };
    let parameters = checked.state_parameters(source);
    // Admission rejoins each Unit arm to an ordinary Terminal target. These
    // selected completions transfer no result, just like a direct Unit return;
    // jumps in the same chain retain their separate edge cleanup.
    let guarded_unit_completion = match &state.terminator {
        CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } => {
            arms.iter().any(|arm| {
                matches!(arm, checked_trees::CheckedGuardedJumpArmPlan::Return(plan)
                    if matches!(plan.return_arm, checked_trees::CheckedConditionalReturnArm::Unit { .. }))
            }) || matches!(fallback, checked_trees::CheckedGuardedJumpFallbackPlan::Return(
                checked_trees::CheckedConditionalReturnArm::Unit { .. }
            ))
        }
        _ => false,
    };
    // Unit and scalar completion transfer no structural result. Both retain
    // the same local custody until return disposal; structural completion
    // instead rejoins the returned owner's transfer before residual cleanup.
    let local_discards = if guarded_unit_completion
        || matches!(
            state.terminator,
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. }
            | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                return_arm: checked_trees::CheckedConditionalReturnArm::Unit { .. }
                    | checked_trees::CheckedConditionalReturnArm::Scalar { .. }
                    | checked_trees::CheckedConditionalReturnArm::Call { .. },
                ..
            }
            // Each arm completes the machine, after its terminal call if any.
            | CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion { .. }
        ) {
        result_custody::local_discards(checked, machine, source, state, None)?
    } else {
        Vec::new()
    };
    let mut drops = Vec::new();
    for (_, event) in checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state.state
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
        })
    {
        if state.operations.iter().filter_map(result_custody::result).any(|result| {
            local_discards.contains(&result.binding_ordinal)
                && matches!(checked.statement_table.statements(source.statement_nodes).get(result.statement_index as usize),
                    Some(StatementNode::LocalData(local)) if event.root == facts::PlaceRoot::Symbol(local.symbol))
        }) {
            continue;
        }
        let index = state
            .structural_parameters
            .iter()
            .position(|parameter| {
                parameters
                    .get(parameter.position as usize)
                    .is_some_and(|source| event.root == facts::PlaceRoot::Symbol(source.symbol))
            })
            .ok_or(LoweringError::Unsupported(
                "Unit return has an unaccounted local drop",
            ))?;
        let parameter = &state.structural_parameters[index];
        if parameter.access != checked_trees::CheckedStructuralAccess::Owned
            || parameter.multiplicity != Multiplicity::Affine
            || event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != PermissionProvenance::Unknown
            || event.obligation_live
            || !event.segments.is_empty()
            || drops.contains(&index)
        {
            return unsupported("Unit return parameter cleanup custody drifted");
        }
        drops.push(index);
    }
    // A whole owned parameter a call consumes needs no exit drop: the call's
    // admission rejoined that move to its authored operand, and its callee
    // owns the value's disposal. Structural return events likewise describe
    // residual custody after the authored value transfer; returning a
    // parameter or moving it into a constructor can remove its exit drop.
    // Source value replay checks those transfers; Terminal independently
    // requires the remaining live frontier, including canonical reverse
    // order, before publication.
    let mut consumed = Vec::new();
    for operation in &state.operations {
        for argument in operation
            .call_structural_arguments()
            .iter()
            .filter(|argument| argument.access == checked_trees::CheckedStructuralAccess::Owned)
        {
            if let Some(index) = argument.source_parameter_index() {
                if !argument.path.is_empty() {
                    return unsupported("Unit return has a partial parameter move");
                }
                consumed.push(index as usize);
            }
        }
    }
    if guarded_unit_completion
        || matches!(
            state.terminator,
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
        )
    {
        // Every other owned affine parameter keeps its exit drop in the
        // source plan. Preserve this admission check; deleting a source drop
        // is not authority to silently omit cleanup. A consumed parameter may
        // keep the drop the source recorded before its consumption or none.
        // A selected parameter source keeps no exit drop of its own: its
        // residual join parameter is disposed by the return splice instead.
        let selected_sources = checked
            .facts
            .flow
            .ownership
            .owned_selections
            .iter()
            .filter(|(_, receipt)| receipt.machine == machine && receipt.state == state.state)
            .flat_map(|(_, receipt)| {
                checked
                    .facts
                    .flow
                    .ownership
                    .selection_sources
                    .span_or_empty(receipt.sources)
                    .iter()
                    .map(|source| source.symbol)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let expected = state
            .structural_parameters
            .iter()
            .enumerate()
            .rev()
            .filter(|(index, parameter)| {
                parameter.access == checked_trees::CheckedStructuralAccess::Owned
                    && parameter.multiplicity == Multiplicity::Affine
                    && !consumed.contains(index)
                    && !parameters
                        .get(parameter.position as usize)
                        .is_some_and(|source| selected_sources.contains(&source.symbol))
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if drops
            .iter()
            .filter(|index| !consumed.contains(index))
            .ne(expected.iter())
        {
            return unsupported("Unit return parameter cleanup roster drifted");
        }
    }
    // A closed-sum edge disposes an affine subject before entering the arm's
    // return block. Its checked exit drop remains required evidence, but must
    // not become a second disposal at that return. A copyable subject has no
    // disposal debt: it must not acquire an affine-drop receipt just because
    // the same case route also handles affine values. Boolean guard
    // observations retain their ordinary return cleanup.
    if let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, .. } = &state.terminator
        && let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            subject.source
    {
        let index = parameter_index as usize;
        let parameter =
            state
                .structural_parameters
                .get(index)
                .ok_or(LoweringError::Unsupported(
                    "case return lost its consumed parameter",
                ))?;
        if subject.access != checked_trees::CheckedStructuralAccess::Owned
            || !subject.path.is_empty()
            || parameter.access != checked_trees::CheckedStructuralAccess::Owned
            || parameter.type_identity != subject.type_identity
            || consumed.contains(&index)
        {
            return unsupported("case return changed its subject disposal custody");
        }
        match parameter.multiplicity {
            Multiplicity::Affine if drops.contains(&index) => consumed.push(index),
            Multiplicity::Unrestricted if !drops.contains(&index) => {}
            _ => return unsupported("case return changed its subject disposal custody"),
        }
    }
    drops.retain(|index| !consumed.contains(index));
    Ok(drops)
}

/// A projected loan must name the authored occurrence, not merely another
/// place with the same leaf type. Existing locals also rejoin their producer.
fn validate_projected_loan_source(
    checked: &CheckedTrees,
    machine_symbol: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    statement_ordinal: u32,
    expression: checked_trees::expression::ExpressionHandle,
    subject: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let (machine, _) =
        crate::expression_preparation::source_custody::authored_state(checked, source.symbol)?;
    if machine.symbol != machine_symbol {
        return unsupported("Unit graph projected loan changed its source machine");
    }
    let (root_symbol, receiver_root, root_type, root_access) = match subject.source {
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter = state
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph projected loan parameter missing",
                ))?;
            let declared = checked
                .state_parameters(source)
                .get(parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph projected loan source position missing",
                ))?;
            (
                declared.symbol,
                declared.is_self.then_some(machine.symbol),
                declared.type_reference,
                parameter.access,
            )
        }
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } => {
            // A loan needs the local's original home, not a producer with an
            // affine-disposal flag. Rejoin every structural result family;
            // exclude call-owned temporaries and automatic return discards.
            let mut results = state
                .operations
                .iter()
                .filter(|operation| {
                    !matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                            operand_source: Some(_),
                            ..
                        }
                    ) && !operation
                        .discardable_structural_result()
                        .is_some_and(|(_, discard)| discard)
                })
                .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
                .filter(|result| result.binding_ordinal == binding_ordinal);
            let result = results.next().ok_or(LoweringError::Unsupported(
                "Unit graph projected loan result missing",
            ))?;
            if results.next().is_some() || result.statement_index >= statement_ordinal {
                return unsupported("Unit graph projected loan result establishment drifted");
            }
            let Some(StatementNode::LocalData(local)) = checked
                .statement_table
                .statements(source.statement_nodes)
                .get(result.statement_index as usize)
            else {
                return unsupported("Unit graph projected loan source local missing");
            };
            result_custody::validate_disposition(checked, machine.symbol, source, local, result)?;
            (
                local.symbol,
                None,
                local.type_reference,
                checked_trees::CheckedStructuralAccess::Owned,
            )
        }
        _ => return unsupported("Unit graph projected loan has no existing source place"),
    };
    let (actual_root, actual_path, actual_access) =
        crate::unit::attached_unit::parameters::source_path(
            checked,
            machine,
            Some(source),
            root_type,
            expression,
        )?;
    use checked_trees::CheckedStructuralAccess::{
        MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow,
    };
    if (actual_root != root_symbol && Some(actual_root) != receiver_root)
        || actual_path != subject.path
        || actual_access.is_some_and(|access| {
            access != subject.access && !(access == MutableBorrow && subject.access == SharedBorrow)
        })
        || !matches!(
            (root_access, subject.access),
            (Owned, SharedBorrow | MutableBorrow | WriteOnlyBorrow)
                | (SharedBorrow, SharedBorrow)
                | (
                    MutableBorrow,
                    SharedBorrow | MutableBorrow | WriteOnlyBorrow
                )
                | (WriteOnlyBorrow, WriteOnlyBorrow)
        )
    {
        return unsupported(
            "Unit graph projected loan differs from its authored root, path, or access",
        );
    }
    Ok(())
}

/// The data declaration a member path walks from its root type: borrows and
/// constraint ascriptions peel away to the owned data shape, and `Self`
/// resolves to the enclosing machine's attached data.
fn nominal_owner(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> Option<&checked_trees::data::DataDefinition> {
    loop {
        match checked.type_reference_table.type_reference(reference) {
            checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                reference = *referee;
            }
            checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type;
            }
            checked_trees::types::TypeReferenceNode::Named { symbol, .. } => {
                if let Some(data) = checked
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == *symbol)
                {
                    return Some(data);
                }
                // `Self` resolves to the enclosing machine's
                // attachment, not the data declaration itself.
                return checked
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == *symbol)
                    .and_then(|machine| {
                        checked
                            .data_definitions()
                            .iter()
                            .find(|data| data.symbol == machine.attached_data_symbol)
                    });
            }
            _ => return None,
        }
    }
}
