//! Exact three-state graph and authored-parameter partition.

use super::*;

pub(super) struct Topology<'a> {
    pub(super) entry: &'a psi_typed_trees::state::State,
    pub(super) leaves: [&'a psi_typed_trees::state::State; 2],
    pub(super) attachment_type_identity: String,
    pub(super) entry_structural_parameters: Vec<CheckedUnitStructuralParameterPlan>,
    pub(super) entry_scalar_parameters: Vec<CheckedStructuralScalarParameterPlan>,
    pub(super) entry_claims: Vec<CheckedUnitEntryClaimPlan>,
    pub(super) entry_bindings: Vec<CheckedScalarBinding>,
    pub(super) entry_binding_initializers: Vec<CheckedScalarExpression>,
    pub(super) leaf_structural_parameters: [Vec<CheckedUnitStructuralParameterPlan>; 2],
    pub(super) leaf_entry_claims: [Vec<CheckedUnitEntryClaimPlan>; 2],
    pub(super) guard: CheckedScalarExpression,
    pub(super) successors: [CheckedStructuralControlSuccessorPlan; 2],
}

pub(super) fn admit<'a>(
    program: &'a TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<Topology<'a>> {
    let [entry, when_true_state, when_false_state] = program.machine_states(machine) else {
        return None;
    };
    let binders = machine_binders(program, machine);
    let mut signatures = Vec::new();
    let mut attachment_type_identity = None;
    for state in [entry, when_true_state, when_false_state] {
        if !is_unit(program, state.return_type) || !program.state_contracts(state).is_empty() {
            return None;
        }
        let (attachment, structural, scalar) =
            structural_scalar_signature(program, shapes, machine, state, &binders, false)?;
        if !only_implicit_reference_self_is_omitted(program, state, &structural, &scalar)
            || attachment_type_identity
                .as_ref()
                .is_some_and(|identity| identity != &attachment)
        {
            return None;
        }
        attachment_type_identity = Some(attachment);
        signatures.push((structural, scalar));
    }
    let [
        (entry_structural_parameters, entry_scalar_parameters),
        (true_structural_parameters, true_scalar_parameters),
        (false_structural_parameters, false_scalar_parameters),
    ] = signatures.as_slice()
    else {
        return None;
    };
    if !true_scalar_parameters.is_empty() || !false_scalar_parameters.is_empty() {
        return None;
    }
    let entry_claims =
        super::custody::exact_claims(program, facts, machine, entry, entry_structural_parameters)?;
    let true_claims = super::custody::exact_claims(
        program,
        facts,
        machine,
        when_true_state,
        true_structural_parameters,
    )?;
    let false_claims = super::custody::exact_claims(
        program,
        facts,
        machine,
        when_false_state,
        false_structural_parameters,
    )?;
    if !super::custody::exact_structural_custody(
        entry_structural_parameters,
        true_structural_parameters,
        false_structural_parameters,
        &entry_claims,
        &true_claims,
        &false_claims,
    ) {
        return None;
    }

    let statements = program.statement_table.statements(entry.statement_nodes);
    let (entry_bindings, entry_binding_initializers, transition_ordinal, when_true, when_false) =
        match statements {
            [
                StatementNode::Transition(when_true),
                StatementNode::Transition(when_false),
            ] => (Vec::new(), Vec::new(), 0, when_true, when_false),
            [
                StatementNode::LocalData(local),
                StatementNode::Transition(when_true),
                StatementNode::Transition(when_false),
            ] if !local.is_mutable && local.initial_value.is_valid() => {
                let primitive_type = program.primitive_type_reference(local.type_reference)?;
                let initializer = facts.values.scalar_expressions.expression_at(
                    entry.symbol,
                    0,
                    CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
                )?;
                if primitive_type != PrimitiveType::U64
                    || !matches!(initializer, CheckedScalarExpression::IntegerLiteral { .. })
                {
                    return None;
                }
                (
                    vec![CheckedScalarBinding {
                        destination: psi_checked_trees::CheckedScalarBindingDestination::Immutable,
                        statement_ordinal: 0,
                        primitive_type,
                        value: CheckedScalarBindingValue::Expression,
                    }],
                    vec![initializer.clone()],
                    1,
                    when_true,
                    when_false,
                )
            }
            _ => return None,
        };
    if when_true.exit != TransitionExit::Ordinary
        || !matches!(when_true.guard, TransitionGuardNode::When(_))
        || when_false.exit != TransitionExit::Ordinary
        || !exact_false_fallback(program, when_true, when_false)
        || when_true.continuation.is_valid()
        || when_false.continuation.is_valid()
    {
        return None;
    }
    let guard = super::guards::exact_guard(
        facts.values.scalar_expressions.expression_at(
            entry.symbol,
            transition_ordinal,
            CheckedScalarExpressionRole::Guard,
        )?,
        entry_scalar_parameters,
        &entry_bindings,
    )?;
    let successors = [
        successor(
            program,
            facts,
            machine,
            entry,
            entry_structural_parameters,
            true_structural_parameters,
            &entry_claims,
            &true_claims,
            transition_ordinal,
            when_true,
            when_true_state.symbol,
            &[],
        )?,
        successor(
            program,
            facts,
            machine,
            entry,
            entry_structural_parameters,
            false_structural_parameters,
            &entry_claims,
            &false_claims,
            transition_ordinal.checked_add(1)?,
            when_false,
            when_false_state.symbol,
            &[],
        )?,
    ];
    Some(Topology {
        entry,
        leaves: [when_true_state, when_false_state],
        attachment_type_identity: attachment_type_identity?,
        entry_structural_parameters: entry_structural_parameters.clone(),
        entry_scalar_parameters: entry_scalar_parameters.clone(),
        entry_claims,
        entry_bindings,
        entry_binding_initializers,
        leaf_structural_parameters: [
            true_structural_parameters.clone(),
            false_structural_parameters.clone(),
        ],
        leaf_entry_claims: [true_claims, false_claims],
        guard,
        successors,
    })
}

fn exact_false_fallback(
    program: &TypedTrees,
    when_true: &psi_typed_trees::statement::TableTransition,
    when_false: &psi_typed_trees::statement::TableTransition,
) -> bool {
    match when_false.guard {
        TransitionGuardNode::Always => true,
        TransitionGuardNode::When(expression) => {
            let TransitionGuardNode::When(true_expression) = when_true.guard else {
                return false;
            };
            let Some(true_subject) = labeled_boolean_guard(program, true_expression, true) else {
                return false;
            };
            let Some(false_subject) = labeled_boolean_guard(program, expression, false) else {
                return false;
            };
            program
                .expression_table
                .expressions_structurally_equal(true_subject, false_subject)
        }
    }
}

fn labeled_boolean_guard(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    expected: bool,
) -> Option<psi_typed_trees::expression::ExpressionHandle> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    (binary.operator == psi_typed_trees::expression::BinaryOperator::Equal
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(value) if *value == expected
        ))
    .then_some(binary.left)
}

pub(super) fn only_implicit_reference_self_is_omitted(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    scalar: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    program
        .state_parameters(state)
        .iter()
        .enumerate()
        .all(|(position, parameter)| {
            structural
                .iter()
                .any(|candidate| candidate.position as usize == position)
                || scalar
                    .iter()
                    .any(|candidate| candidate.source_position as usize == position)
                || (parameter.is_self && is_reference(program, parameter.type_reference))
        })
}

pub(super) fn successor(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    source_state: &psi_typed_trees::state::State,
    source_parameters: &[CheckedUnitStructuralParameterPlan],
    target_parameters: &[CheckedUnitStructuralParameterPlan],
    source_claims: &[CheckedUnitEntryClaimPlan],
    target_claims: &[CheckedUnitEntryClaimPlan],
    ordinal: u32,
    transition: &psi_typed_trees::statement::TableTransition,
    expected: SymbolHandle,
    admitted_local_discards: &[SymbolHandle],
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    if path.symbol != expected {
        return None;
    }
    let arguments = program.statement_table.expression_handles(*arguments);
    let transfers = match (
        source_parameters,
        target_parameters,
        source_claims,
        target_claims,
        arguments,
    ) {
        ([], [], [], [], []) => Vec::new(),
        ([source], [target], [source_claim], [target_claim], [argument]) => {
            let place = crate::flow::canonical_place_from_expression_in_state(
                program,
                source_state.symbol,
                usize::try_from(ordinal).ok()?,
                *argument,
            )?;
            let psi_facts::PlaceRoot::Symbol(root) = place.root else {
                return None;
            };
            let source_symbol = program
                .state_parameters(source_state)
                .get(source.position as usize)?
                .symbol;
            if root != source_symbol
                || !place.segments.is_empty()
                || source.type_identity != target.type_identity
                || source.multiplicity != target.multiplicity
                || source.access != target.access
                || !super::custody::exact_claim_alias_events(
                    facts,
                    machine,
                    source_state,
                    ordinal,
                    expected,
                    source_symbol,
                    source_claim,
                    target_claim,
                )
            {
                return None;
            }
            vec![CheckedStructuralControlTransferPlan {
                source_parameter_index: 0,
                target_parameter_index: 0,
            }]
        }
        _ => return None,
    };
    if admitted_local_discards.is_empty() {
        let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
            machine.symbol,
            source_state.symbol,
            ordinal,
        )?;
        if cleanup.target_state != expected
            || !cleanup
                .trivial_affine_discard_parameter_positions
                .is_empty()
        {
            return None;
        }
    } else if admitted_local_discards.len() != 1
        || !source_parameters.is_empty()
        || !target_parameters.is_empty()
        || !source_claims.is_empty()
        || !target_claims.is_empty()
        || !super::super::types::return_unit_affine_discards(
            program,
            facts,
            machine.symbol,
            source_state.symbol,
            source_parameters,
            program.state_parameters(source_state),
            &[],
            admitted_local_discards,
        )?
        .is_empty()
    {
        return None;
    }
    Some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: ordinal,
        target_state: expected,
        transfers,
        scalar_arguments: Vec::new(),
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}
