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
    validate_bindings(checked, plan, source, state, transition, edge, ordinal, &[])?;
    validate_cleanup(checked, plan, source, state, edge)
}

/// Case dispatch validates local-result cleanup separately from parameter cleanup.
pub(super) fn validate_bindings(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    transition: &TableTransition,
    edge: &CheckedStructuralControlSuccessorPlan,
    ordinal: usize,
    payloads: &[checked_trees::CheckedClosedSumPayloadTransferPlan],
) -> Result<(), LoweringError> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = checked.statement_table.transition_target(transition.target)
    else {
        return unsupported("Unit graph edge has no named source target");
    };
    let authored_target = if path.symbol == plan.machine {
        plan.states[0].state
    } else {
        path.symbol
    };
    if edge.statement_ordinal as usize != ordinal
        || edge.target_state != authored_target
        || transition.exit != TransitionExit::Ordinary
        || transition.continuation.is_valid()
    {
        return unsupported("Unit graph successor disagrees with source control");
    }
    // Every edge's owned-parameter discards are exactly those its checked
    // cleanup evidence names; an edge without evidence discards none.
    //
    // The evidence counts a parameter as dying on the edge whenever the
    // edge's transition moves none of it, and a closed-sum dispatch moves no
    // place: its arms read payloads out of the subject. Terminal ownership
    // accounts every owned obligation on an edge exactly once, and the case
    // dispatch itself is the subject's explicit terminal consumption, so the
    // producer's edge evidence omits it by construction. Naming it here again
    // would be a second disposition. The
    // opposite reading, that the arm transfers the payload and discards the
    // shell, would need the dispatch to leave the subject live, which the
    // StructuralCase terminator does not.
    let evidence = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_edge(plan.machine, state.state, edge.statement_ordinal)
        .map(|cleanup| cleanup.trivial_affine_discard_parameter_positions.to_vec())
        .unwrap_or_default();
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
    let explicit_argument_position = |position: u32| {
        position as usize
            - target
                .structural_parameters
                .iter()
                .filter(|parameter| parameter.is_self && parameter.position < position)
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
                    if name.symbol == source.symbol
                        && name.head_symbol == source.symbol
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
            let mut matching =
                state
                    .operations
                    .iter()
                    .filter_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::StructuralCall {
                            result,
                            discard_result_on_return: false,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                            result,
                            discard_result_on_return: false,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                            result,
                            discard_result_on_return: false,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::EstablishViewSubslice {
                            result, ..
                        } if result.binding_ordinal == binding_ordinal => Some(result),
                        _ => None,
                    });
            let result = matching.next().ok_or(LoweringError::Unsupported(
                "Unit graph transferred result missing",
            ))?;
            // An owned local moves into the target; a view local lends its
            // shared view, which owns nothing and so leaves no partition.
            let shared_view = target.access == checked_trees::CheckedStructuralAccess::SharedBorrow;
            if matching.next().is_some()
                || result.statement_index >= edge.statement_ordinal
                || transfer.target_parameter_index as usize != position
                || target.is_self
                || !(target.access == checked_trees::CheckedStructuralAccess::Owned
                    || (shared_view && result.multiplicity == Multiplicity::Unrestricted))
                || !target.qualifications.is_empty()
                || target.multiplicity != result.multiplicity
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
            let argument_position = explicit_argument_position(target.position);
            let expression = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph result argument missing",
                ))?;
            if local.is_mutable
                || !matches!(checked.expression_table.expression(*expression), ExpressionNode::Name(path) if path.symbol == local.symbol && path.head_symbol == local.symbol && checked.expression_table.name_path_members(path.members).len() == 1)
            {
                return unsupported("Unit graph result argument lost its actual local");
            }
            if shared_view {
                view_local_result(checked, source, state, local.symbol, edge.statement_ordinal)?;
            } else {
                result_custody::validate(checked, plan.machine, source, local, result)?;
            }
            continue;
        }
        if let Some((root, _, expression)) = subslices::transfer_range(&transfer.source) {
            // A view subslice narrows a whole view parameter or a view local
            // the state body established; either way the target receives a
            // fresh shared view of the source's own type. The range itself is
            // replayed at its edge site when the edge emits it.
            if arguments.get(target.position as usize) != Some(&expression) {
                return unsupported("Unit graph subslice disagrees with its source argument");
            }
            let source_type = match root {
                checked_trees::CheckedStorageRoot::Parameter { index } => {
                    let source = state.structural_parameters.get(index as usize).ok_or(
                        LoweringError::Unsupported("Unit graph borrowed transfer source missing"),
                    )?;
                    if source.access != target.access || source.multiplicity != target.multiplicity
                    {
                        return unsupported("Unit graph borrowed transfer type or order drifted");
                    }
                    &source.type_identity
                }
                checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                    view_local_result(checked, source, state, symbol, edge.statement_ordinal)?
                }
            };
            if transfer.target_parameter_index as usize != position
                || target.is_self
                || target.access != checked_trees::CheckedStructuralAccess::SharedBorrow
                || target.multiplicity != Multiplicity::Unrestricted
                || *source_type != target.type_identity
            {
                return unsupported("Unit graph borrowed transfer type or order drifted");
            }
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
            if super::cases::identity(variant) != *case_identity {
                return unsupported("Unit graph case-payload case identity drifted");
            }
            let Some(field) = checked.data_payload_fields(variant).iter().find(|field| {
                field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned())
                    == *field_identity
            }) else {
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
            // The subject resolves to a retained source parameter and a
            // member path ending at the sum owning the selected case.
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } = subject.source
            else {
                return unsupported("Unit graph case-payload subject unsupported");
            };
            let source_parameter = state
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph case-payload subject missing",
                ))?;
            if subject.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
                return unsupported("Unit graph case-payload subject custody drifted");
            }
            // The subject's member path walks from its own root type; borrows
            // and constraint ascriptions peel away to the owned data shape.
            fn nominal_owner(
                checked: &CheckedTrees,
                mut reference: checked_trees::types::TypeReferenceHandle,
            ) -> Option<&checked_trees::data::DataDefinition> {
                loop {
                    match checked.type_reference_table.type_reference(reference) {
                        checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                            reference = *referee;
                        }
                        checked_trees::types::TypeReferenceNode::Constrained {
                            base_type, ..
                        } => {
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
            let mut reference = source_parameters
                .get(source_parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph case-payload parameter missing",
                ))?
                .type_reference;
            let Some(mut owner) = nominal_owner(checked, reference) else {
                return unsupported("Unit graph case-payload subject is not nominal");
            };
            for segment in &subject.path {
                let checked_trees::CheckedUnitStructuralPathSegment::Field(identity) = segment
                else {
                    return unsupported("Unit graph case-payload path is not a member walk");
                };
                let Some(member) = checked.data_members(owner).iter().find_map(|member| {
                    let checked_trees::data::DataMember::Field(field) = member else {
                        return None;
                    };
                    (field
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
            if checked.normalized_type_identity(reference).into_string() != subject.type_identity
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
            checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload { .. } => {
                return unsupported("Unit graph case-payload was not independently rejoined");
            }
        };
        let source = state
            .structural_parameters
            .get(source_index as usize)
            .ok_or(LoweringError::Unsupported(
                "Unit graph borrowed transfer source missing",
            ))?;
        if transfer.target_parameter_index as usize != position
            || source.type_identity != target.type_identity
            || source.access != target.access
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
                    if source != target {
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
            checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload { .. } => {
                return unsupported("Unit graph case-payload was not independently rejoined");
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
/// established before `statement`: the unique shared view result an
/// `as_slice` loan or a subslice binding published for that `let`.
fn view_local_result<'a>(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    state: &'a CheckedComposedUnitControlStatePlan,
    symbol: symbols::SymbolHandle,
    statement: u32,
) -> Result<&'a String, LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    let mut matching = state.operations.iter().filter_map(|operation| {
        let result = match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. } => result,
            _ => return None,
        };
        matches!(
            statements.get(result.statement_index as usize),
            Some(checked_trees::statement::StatementNode::LocalData(local))
                if local.symbol == symbol && !local.is_mutable
        )
        .then_some(result)
    });
    let result = matching.next().ok_or(LoweringError::Unsupported(
        "Unit graph view local has no establishment",
    ))?;
    if matching.next().is_some()
        || result.statement_index >= statement
        || result.multiplicity != Multiplicity::Unrestricted
    {
        return unsupported("Unit graph view local establishment drifted");
    }
    Ok(&result.type_identity)
}

fn validate_cleanup(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    edge: &CheckedStructuralControlSuccessorPlan,
) -> Result<(), LoweringError> {
    result_custody::validate_disposition_roster(checked, plan.machine, source, state)?;
    result_custody::local_discards(checked, plan.machine, source, state, Some(edge))?;
    let cleanup = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_edge(plan.machine, state.state, edge.statement_ordinal)
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

/// A guarded second arm stands in for the unconditional fallback only when it
/// holds exactly where the first guard fails.
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
    if crate::expression_preparation::source_custody::guard_complement::complementary(
        checked,
        state,
        guard_ordinal,
    )? {
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
    let local_discards = if matches!(
        state.terminator,
        CheckedComposedUnitControlTerminatorPlan::ReturnUnit
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
        let arguments = match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                structural_arguments,
                ..
            } => structural_arguments.as_slice(),
            _ => &[],
        };
        for argument in arguments
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
    if matches!(
        state.terminator,
        CheckedComposedUnitControlTerminatorPlan::ReturnUnit
    ) {
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
    drops.retain(|index| !consumed.contains(index));
    Ok(drops)
}
