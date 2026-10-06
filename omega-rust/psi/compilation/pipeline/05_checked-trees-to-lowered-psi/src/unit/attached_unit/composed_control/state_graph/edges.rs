//! Authored successor operands and ownership cleanup at edges and returns.
//!
//! Cleanup owner rosters use the shared ownership-facts query: a root can die
//! on several mutually exclusive exits without being disposed twice. Edge
//! bindings and return transfers select the actual residual cleanup below.
use super::super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::super::{
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, Multiplicity, unsupported,
};
use super::super::{CheckedTrees, LoweringError};
use super::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedStructuralControlSuccessorPlan, result_custody, subslices,
};
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::{
    StatementNode, TableTransition, TransitionExit, TransitionTargetNode,
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
    validate_cleanup(checked, state, edge)
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
    let source_parameters = checked.state_parameters(source);
    let target_formals = crate::declarations::state_declaration(checked, edge.target_state)
        .map(|(_, state)| checked.state_parameters(state))?;
    let validate_argument =
        |argument_position: u32, source_position: u32| {
            let argument_position = argument_position as usize;
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph source argument missing",
                ))?;
            let source = source_parameters.get(source_position as usize).ok_or(
                LoweringError::Unsupported("Unit graph source parameter missing"),
            )?;
            let source_symbol = source.symbol;
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
    for (target, transfer) in target.structural_parameters.iter().zip(&edge.transfers) {
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
            binding_ordinal,
        } = transfer.source
        {
            // Bindings establish availability; optional disposal metadata
            // only excludes a result that its automatic cleanup retires.
            let result = state
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
                .find(|result| result.binding_ordinal == binding_ordinal)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph transferred result missing",
                ))?;
            // An owned local moves into the target; a view local lends its
            // shared view, which owns nothing and so leaves no partition. A
            // `&mut [T]` view local's affine binding is the exclusive loan
            // custody its `let` established; the target's mutable view
            // parameter continues that same loan.
            //
            // The move/copy case reads no binding mutability. `mut` is the
            // source-level write permission on the name, not an ownership
            // category: the local's custody at this edge is the result
            // binding's multiplicity, this edge's cleanup evidence, and the
            // checked ownership roster, all of which the plan already carries
            // and the Terminal ownership verifier independently reconstructs.
            // Re-deriving a refusal from the authored `mut` token refused
            // exactly the shape the source contract requires an implementation
            // to support -- a scratch buffer filled through `&mut` in one state
            // and passed on as a state argument, rather than parked in a
            // persistent context record.
            let expression =
                arguments
                    .get(target.position as usize)
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
                    || !crate::content_classes::plain_owned_with_numeric_constraints(
                        checked,
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
                *expression
            };
            if !matches!(checked.expression_table.expression(actual), ExpressionNode::Name(path) if path.symbol == local.symbol && path.head_symbol == local.symbol && checked.expression_table.name_path_members(path.members).len() == 1)
            {
                return unsupported("Unit graph result argument lost its actual local");
            }
            if shared_view || mutable_view {
                validate_view_local(checked, source, state, local.symbol)?;
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
            // narrowing. The range itself is replayed at its edge site when
            // the edge emits it.
            // Retained positions count the receiver formal; the authored
            // argument list does not.
            if arguments.get(target.position as usize) != Some(&expression) {
                return unsupported("Unit graph subslice disagrees with its source argument");
            }
            if let ExpressionNode::Borrow(_) = checked.expression_table.expression(expression)
                && validation::reference_shell(
                    &checked.typed,
                    target_formals[target.position as usize].type_reference,
                )
                .is_none()
            {
                return unsupported("Unit graph range target is not a reference");
            }
            match root {
                checked_trees::CheckedStorageRoot::Parameter { index } => {
                    let source = state.structural_parameters.get(index as usize).ok_or(
                        LoweringError::Unsupported("Unit graph borrowed transfer source missing"),
                    )?;
                    // The field owner's root only lends the array: it must be
                    // readable and unqualified. The projection stays inside
                    // the rooted subject's own storage: members of records and
                    // selected case payloads, and array elements those
                    // members name. A reference crossing would lend storage
                    // another place owns, so it stays refused here.
                    if !collection.is_empty()
                        && (!collection.iter().all(|segment| {
                            matches!(
                                segment,
                                checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                                    | checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                                    | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                                        _
                                    )
                                    | checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                                        _
                                    )
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
                            || !source.projected_qualifications.is_empty())
                    {
                        return unsupported(
                            "Unit graph subslice field owner cannot lend the requested view",
                        );
                    }
                }
                checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                    if !collection.is_empty() {
                        return unsupported("Unit graph subslice field range lost its owner");
                    }
                    validate_view_local(checked, source, state, symbol)?;
                }
                checked_trees::CheckedStorageRoot::OwnedLocal { .. } => {
                    return unsupported(
                        "Unit graph borrowed transfer source is an owned local, not a view",
                    );
                }
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
            if establishment.structural_result_binding().is_none() {
                return unsupported("Unit graph established argument has no result");
            }
            let argument_position = target.position as usize;
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
                    } => {}
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
                    checked_trees::CheckedUnitEffectOperationPlan::Call(_) => {}
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
                } if *value == root.root => {}
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
                }
                _ => return unsupported("Unit graph argument is not a fresh value producer"),
            }
            continue;
        }
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
            subject,
            ..
        } = &transfer.source
        {
            // Rejoin the argument's declared reference correspondence, not
            // its spelling: a shared field loan needs no explicit `&` token.
            // Its access matches the loan's and the target's, the projected
            // identity is the target's, the path carries only the segments
            // the admit side admits, and the subject roots at a parameter or
            // a rejoinable result binding.
            let argument_position = target.position as usize;
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph loan argument missing",
                ))?;
            if target_formals.get(target.position as usize).is_none() {
                return unsupported("Unit graph loan target declaration missing");
            }
            validate_projected_loan_source(checked, source, state, *expression, subject)?;
            continue;
        }
        // A case payload transfer names its subject, case and field in the
        // plan; the edge emitter copies that field from the selected case.
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload { .. } =
            &transfer.source
        {
            continue;
        }
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedValue {
            subject,
        } = &transfer.source
        {
            // The checked path names a subtree of an established owner.
            // Copyable leaves materialize separately; affine leaves transfer
            // this path and leave the selected complement for edge cleanup.
            let argument_position = target.position as usize;
            let _expression =
                arguments
                    .get(argument_position)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph projected argument missing",
                    ))?;
            match subject.source {
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
                    if source_parameters.get(retained.position as usize).is_none() {
                        return unsupported("Unit graph projected parameter missing");
                    }
                }
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                } => {
                    if !state
                        .operations
                        .iter()
                        .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
                        .any(|result| result.binding_ordinal == binding_ordinal)
                    {
                        return unsupported("Unit graph projected result missing");
                    }
                }
                _ => return unsupported("Unit graph projected subject unsupported"),
            }
            continue;
        }
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceLiteral {
            ..
        } = &transfer.source
        {
            // A literal argument has no custody root: the edge establishes
            // the authored octets into immutable storage the borrowed
            // byte-view target reads. The authored operand must be a string
            // literal.
            let argument_position = target.position as usize;
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph literal argument missing",
                ))?;
            let ExpressionNode::String(_) = checked.expression_table.expression(*expression) else {
                return unsupported("Unit graph literal argument lost its bytes");
            };
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
            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedValue {
                ..
            } => {
                return unsupported("Unit graph projected argument was not independently rejoined");
            }
        };
        let source = state
            .structural_parameters
            .get(source_index as usize)
            .ok_or(LoweringError::Unsupported(
                "Unit graph borrowed transfer source missing",
            ))?;
        match transfer.source {
            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult {
                ..
            } => return unsupported("Unit graph result was not independently rejoined"),
            checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { .. } => {
                validate_argument(target.position, source.position)?;
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
            checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedValue {
                ..
            } => {
                return unsupported("Unit graph projected argument was not independently rejoined");
            }
        }
    }
    for ((_, target), transfer) in target
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
        match transfer.source {
            checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index } => {
                let source = state.scalar_parameters.get(index as usize).ok_or(
                    LoweringError::Unsupported("Unit graph scalar transfer source missing"),
                )?;
                validate_argument(target.source_position, source.source_position)?;
            }
            checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression => {
                super::scalars::successor_value(checked, state, edge, transfer)?;
            }
        }
    }
    // Proof-only lanes carry the same authored expression custody as the
    // retained arguments; erased formals never bind a runtime parameter.
    for (_, transfer) in target
        .erased_scalar_parameters
        .iter()
        .zip(&edge.erased_arguments)
    {
        let (_, _) = checked
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
    }
    Ok(())
}

/// The view local `symbol` must have an establishment in the state body: an
/// `as_slice`/`as_mut_slice` loan or a subslice binding published for that
/// `let`.
fn validate_view_local(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    symbol: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    let established = state.operations.iter().any(|operation| {
        let result = match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. } => result,
            _ => return false,
        };
        matches!(
            statements.get(result.statement_index as usize),
            Some(checked_trees::statement::StatementNode::LocalData(local))
                if local.symbol == symbol && !local.is_mutable
        )
    });
    if !established {
        return unsupported("Unit graph view local has no establishment");
    }
    Ok(())
}

fn validate_cleanup(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    edge: &CheckedStructuralControlSuccessorPlan,
) -> Result<(), LoweringError> {
    // Stage 04 selected the ordered whole/residual action stream. A second
    // whole-local inventory cannot represent a partially transferred root.
    let exit = super::cleanup::coordinate(checked, state, edge.statement_ordinal as usize, false)?;
    if state
        .normal_exit_cleanups
        .iter()
        .filter(|cleanup| cleanup.exit == exit)
        .count()
        != 1
    {
        return unsupported("Unit graph edge has no exact normal cleanup plan");
    }
    Ok(())
}

/// Reconstruct the remaining whole-parameter drops at an ordinary return.
/// Call consumption and return disposal are separate uses of the same source root.
pub(super) fn return_discards(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<Vec<usize>, LoweringError> {
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
    let early_discards = result_custody::statement_discards(state);
    let mut drops = Vec::new();
    for event in checked
        .facts
        .flow
        .ownership
        .state_exit_cleanup_roots(machine, state.state)
        .ok_or(LoweringError::Unsupported(
            "Unit return exit cleanup is inconsistent",
        ))?
    {
        if state.operations.iter().filter_map(result_custody::result).any(|result| {
            (local_discards.contains(&result.binding_ordinal)
                || early_discards.contains(&result.binding_ordinal))
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
        drops.push(index);
    }
    // The source query joins cleanup owners across mutually exclusive exits
    // in first-occurrence order. An earlier branch can transfer a later
    // parameter, so that union is not an executable disposal schedule.
    // Order the authenticated owners by declaration before checking the
    // roster and subtracting consumed roots; surviving owners keep that order.
    drops.sort_unstable_by_key(|parameter_index| {
        std::cmp::Reverse(state.structural_parameters[*parameter_index].position)
    });
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
        if parameter.multiplicity == Multiplicity::Affine && drops.contains(&index) {
            consumed.push(index);
        }
    }
    drops.retain(|index| !consumed.contains(index));
    Ok(drops)
}

/// A projected loan must name the authored occurrence, not merely another
/// place with the same leaf type. Existing locals also rejoin their producer.
fn validate_projected_loan_source(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    expression: checked_trees::expression::ExpressionHandle,
    subject: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let (machine, _) = crate::declarations::state_declaration(checked, source.symbol)?;
    let root_type = match subject.source {
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter = state
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph projected loan parameter missing",
                ))?;
            checked
                .state_parameters(source)
                .get(parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph projected loan source position missing",
                ))?
                .type_reference
        }
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } => {
            // A loan needs the local's original home, not a producer with an
            // affine-disposal flag. Rejoin every structural result family;
            // exclude call-owned temporaries and automatic return discards.
            let result = state
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
                .find(|result| result.binding_ordinal == binding_ordinal)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph projected loan result missing",
                ))?;
            let Some(StatementNode::LocalData(local)) = checked
                .statement_table
                .statements(source.statement_nodes)
                .get(result.statement_index as usize)
            else {
                return unsupported("Unit graph projected loan source local missing");
            };
            local.type_reference
        }
        _ => return unsupported("Unit graph projected loan has no existing source place"),
    };
    crate::unit::attached_unit::parameters::source_path(
        checked,
        machine,
        Some(source),
        root_type,
        expression,
    )?;
    Ok(())
}
