use std::collections::{BTreeMap, BTreeSet};

use psi_checked_trees::{
    CheckFacts, CheckedScalarBinding, CheckedScalarBindingValue, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralResultPlan,
    CheckedStructuralReturnMachinePlan, CheckedStructuralReturnPlans,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarParameterPlan,
    CheckedStructuralScalarReturnMachinePlan, CheckedStructuralScalarReturnPlans,
    CheckedStructuralUnitControlMachinePlan, CheckedStructuralUnitControlPlans,
    CheckedStructuralUnitControlStatePlan, CheckedStructuralUnitControlTerminatorPlan,
    CheckedUnitBoundaryMachinePlan, CheckedUnitCallCoordinate, CheckedUnitClaimTransferPlan,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, CheckedUnitEffectPlans,
    CheckedUnitEntryClaimPlan, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralTypePlan, ContractProofFactKind, ContractProofFactOwner,
};
use psi_language_semantics::{
    CarryPolicy, MachineSupplyMode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, SemanticDomainId,
};
use psi_symbols::{BuiltinFunction, SymbolHandle};
use psi_typed_trees::{
    TypedTrees,
    data::{DataMember, DataShapeKind},
    domain::ProofFact,
    expression::ExpressionNode,
    signature::{SignatureContractKind, StateParameter},
    statement::{StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode},
    types::{PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode},
};

/// Build the first general structural/Unit terminal plan after ownership and
/// carry checking have recorded their authoritative facts. Unsupported shapes
/// are omitted as a closed unit; callers therefore cannot accidentally lower a
/// root whose transitive helper or boundary settlement was only partly known.
pub(crate) fn build_checked_unit_effect_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedUnitEffectPlans {
    let mut shapes = ShapeCollector::new(program);
    let boundary_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode.is_boundary_declaration())
        .filter_map(|machine| build_boundary_machine(program, facts, &mut shapes, machine))
        .collect::<Vec<_>>();
    let boundary_symbols = boundary_machines
        .iter()
        .map(|plan| plan.machine)
        .collect::<Vec<_>>();
    let mut candidates = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| build_checked_machine(program, facts, &mut shapes, machine))
        .collect::<Vec<_>>();

    loop {
        let checked_symbols = candidates
            .iter()
            .map(|plan| plan.machine)
            .collect::<Vec<_>>();
        let old_len = candidates.len();
        candidates.retain(|plan| {
            plan.operations.iter().all(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => {
                    checked_symbols.contains(target_machine)
                }
                CheckedUnitEffectOperationPlan::BoundaryCallUnit { target_machine, .. } => {
                    boundary_symbols.contains(target_machine)
                }
                CheckedUnitEffectOperationPlan::PortWrite { .. }
                | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => true,
            })
        });
        if candidates.len() == old_len {
            break;
        }
    }
    let retained_type_identities = boundary_machines
        .iter()
        .flat_map(|plan| {
            std::iter::once(plan.attachment_type_identity.as_str()).chain(
                plan.structural_parameters
                    .iter()
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        })
        .chain(candidates.iter().flat_map(|plan| {
            std::iter::once(plan.attachment_type_identity.as_str()).chain(
                plan.structural_parameters
                    .iter()
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        }))
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained_type_identities);

    CheckedUnitEffectPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: {
            shapes.domains.sort_by_key(|domain| domain.domain.0);
            shapes.domains
        },
        boundary_machines,
        machines: candidates,
    }
}

/// Build the exact checked carrier for `T in D -> T in D` whole-root
/// passthrough. Every wider ownership or control shape is omitted atomically.
pub(crate) fn build_checked_structural_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| build_structural_return_machine(program, facts, &mut shapes, machine))
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|plan| {
            std::iter::once(plan.attachment_type_identity.as_str())
                .chain(
                    plan.structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(std::iter::once(plan.result.type_identity.as_str()))
        })
        .collect::<BTreeSet<_>>();
    let retained_domains = machines
        .iter()
        .flat_map(|plan| {
            plan.structural_parameters
                .iter()
                .flat_map(|parameter| &parameter.qualifications)
                .chain(&plan.result.qualifications)
                .map(|domain| domain.0)
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    shapes
        .domains
        .retain(|domain| retained_domains.contains(&domain.domain.0));
    shapes.domains.sort_by_key(|domain| domain.domain.0);
    CheckedStructuralReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: shapes.domains,
        machines,
    }
}

fn build_structural_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedStructuralReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [StatementNode::Expression(return_expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    if !program.machine_contracts(machine).is_empty() {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
    let input = structural_parameters.first()?;
    if !matches!(structural_parameters.len(), 1 | 2)
        || input.multiplicity != Multiplicity::Linear
        || input.is_self
        || structural_parameters.get(1).is_some_and(|discarded| {
            discarded.multiplicity != Multiplicity::Affine || discarded.is_self
        })
    {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let source_parameter = source_parameters.get(input.position as usize)?;
    let ExpressionNode::Name(path) = program.expression_table.expression(*return_expression) else {
        return None;
    };
    if path.symbol != source_parameter.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let result_type_identity = shapes.add_type(state.return_type, &binders, &[])?;
    let result_qualifications =
        parameter_qualifications(program, shapes, state.return_type, &binders)?;
    if result_type_identity != input.type_identity
        || result_qualifications != input.qualifications
        || crate::checks::type_multiplicity(program, state.return_type) != Multiplicity::Linear
        || !state_contracts_are_exact_parameter_qualifications(
            program,
            state,
            source_parameter,
            &input.qualifications,
        )
    {
        return None;
    }
    let checked_entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        source_parameters,
    )?;
    let [entry_claim] = checked_entry_claims.as_slice() else {
        return None;
    };
    if entry_claim.parameter_index != 0
        || !entry_claim.field_path.is_empty()
        || entry_claim.carry != CarryPolicy::STRICT
    {
        return None;
    }
    let trivial_affine_discards = return_unit_affine_discards(
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        source_parameters,
        &[],
    )?;
    let expected_discards: &[u32] = if structural_parameters.len() == 2 {
        &[1]
    } else {
        &[]
    };
    if trivial_affine_discards != expected_discards {
        return None;
    }
    let outcome_maps = facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .filter(|(_, map)| map.machine_symbol == machine.symbol && map.state_symbol == state.symbol)
        .map(|(_, map)| map)
        .collect::<Vec<_>>();
    let [outcome_map] = outcome_maps.as_slice() else {
        return None;
    };
    let [outcome] = facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_or_empty(outcome_map.entries)
    else {
        return None;
    };
    let psi_checked_trees::FlowClaimOutcomeSource::Input {
        parameter_symbol,
        segments: input_segments,
    } = outcome.source
    else {
        return None;
    };
    if parameter_symbol != source_parameter.symbol
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(input_segments)
            .is_empty()
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(outcome.output_segments)
            .is_empty()
    {
        return None;
    }
    let reshuffles = facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == machine.symbol && fact.state_symbol == state.symbol)
        .collect::<Vec<_>>();
    let [reshuffle] = reshuffles.as_slice() else {
        return None;
    };
    if reshuffle.claim_identity != entry_claim.claim_identity
        || reshuffle.input_parameter_symbol != source_parameter.symbol
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(reshuffle.input_segments)
            .is_empty()
        || !facts
            .flow
            .ownership
            .segments
            .span_or_empty(reshuffle.output_segments)
            .is_empty()
    {
        return None;
    }
    Some(CheckedStructuralReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        returned_parameter_index: 0,
        result: CheckedStructuralResultPlan {
            type_identity: result_type_identity,
            multiplicity: Multiplicity::Linear,
            qualifications: result_qualifications,
        },
        entry_claim: entry_claim.clone(),
        trivial_affine_discards,
        transferred_claim: entry_claim.claim_identity,
    })
}

fn state_contracts_are_exact_parameter_qualifications(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    parameter: &StateParameter,
    expected_domains: &[SemanticDomainId],
) -> bool {
    let mut actual_domains = Vec::new();
    for contract in program.state_contracts(state) {
        if contract.token_count != 0 || contract.kind != SignatureContractKind::Requires {
            return false;
        }
        let [ProofFact::Membership(membership)] = program.proof_facts.span_or_empty(contract.facts)
        else {
            return false;
        };
        let ExpressionNode::Name(path) = program.expression_table.expression(membership.value)
        else {
            return false;
        };
        if path.symbol != parameter.symbol
            || program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return false;
        }
        let Some(domain) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == membership.domain_symbol)
        else {
            return false;
        };
        actual_domains.push(domain.semantic_id);
    }
    actual_domains.sort_by_key(|domain| domain.0);
    actual_domains == expected_domains
}

/// Compose the exact cleanup rows with source-independent structural
/// signatures and whole-parameter transfer maps for the first terminal
/// structural-control producer.
pub(crate) fn build_checked_structural_unit_control_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralUnitControlPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_structural_unit_control_machine(program, facts, &mut shapes, machine)
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|machine| {
            std::iter::once(machine.attachment_type_identity.as_str()).chain(
                machine
                    .states
                    .iter()
                    .flat_map(|state| &state.structural_parameters)
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedStructuralUnitControlPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
    }
}

/// Bind one closed scalar return to an exact affine structural entry frontier.
/// This is deliberately separate from the primitive scalar graph: structural
/// parameters are custody, not fake scalar arguments.
pub(crate) fn build_checked_structural_scalar_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralScalarReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_structural_scalar_return_machine(program, facts, &mut shapes, machine)
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|machine| {
            std::iter::once(machine.attachment_type_identity.as_str()).chain(
                machine
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedStructuralScalarReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
    }
}

fn build_structural_scalar_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedStructuralScalarReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !program.state_contracts(state).is_empty()
        || facts.flow.ownership.permissions.iter().any(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
        })
    {
        return None;
    }
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    if !facts
        .service_reaches
        .rows
        .services(flow.service_reach.direct)
        .is_empty()
        || !facts
            .service_reaches
            .rows
            .services(flow.service_reach.transitive)
            .is_empty()
    {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters, scalar_parameters) =
        structural_scalar_signature(program, shapes, machine, state, &binders)?;
    if structural_parameters.is_empty()
        || structural_parameters.len() + scalar_parameters.len()
            != program.state_parameters(state).len()
        || structural_parameters.iter().any(|parameter| {
            parameter.is_self
                || parameter.multiplicity != Multiplicity::Affine
                || !parameter.qualifications.is_empty()
        })
    {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let binding_count = statements
        .iter()
        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    let bindings = statements[..binding_count]
        .iter()
        .enumerate()
        .map(|(statement_index, statement)| {
            let StatementNode::LocalData(local) = statement else {
                unreachable!("binding prefix contains only local data")
            };
            if local.is_mutable || !local.initial_value.is_valid() {
                return None;
            }
            let statement_ordinal = u32::try_from(statement_index).ok()?;
            let binding_ordinal = statement_ordinal;
            let primitive_type = program.primitive_type_reference(local.type_reference)?;
            let expression = facts.values.scalar_expressions.expression_at(
                state.symbol,
                statement_ordinal,
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
            )?;
            let branch_free = is_branch_free_structural_scalar_expression(
                expression,
                scalar_parameters.len(),
                statement_index,
            );
            let short_circuit_boolean = primitive_type == PrimitiveType::Bool
                && matches!(expression, CheckedScalarExpression::Boolean(expression)
                if checked_boolean_contains_short_circuit(expression)
                    && is_structural_boolean_return_expression(
                        expression,
                        scalar_parameters.len(),
                        statement_index,
                    ));
            (branch_free || short_circuit_boolean).then_some(CheckedScalarBinding {
                statement_ordinal,
                primitive_type,
                value: CheckedScalarBindingValue::Expression,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let [StatementNode::Expression(_)] = &statements[binding_count..] else {
        return None;
    };
    let return_statement_ordinal = u32::try_from(binding_count).ok()?;
    let result_type = program.primitive_type_reference(state.return_type)?;
    let return_expression = facts.values.scalar_expressions.expression_at(
        state.symbol,
        return_statement_ordinal,
        CheckedScalarExpressionRole::Return,
    )?;
    if !is_structural_scalar_return_expression(
        return_expression,
        scalar_parameters.len(),
        binding_count,
    ) {
        return None;
    }
    Some(CheckedStructuralScalarReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        scalar_parameters,
        bindings,
        result_type,
        return_statement_ordinal,
        trivial_affine_discard_parameter_positions:
            super::terminal_cleanup::checked_whole_affine_discard_parameters(
                program,
                facts,
                machine.symbol,
                state,
            )?
            .into_iter()
            .map(|(_, position)| position)
            .collect(),
    })
}

fn checked_boolean_contains_short_circuit(
    expression: &psi_checked_trees::CheckedBooleanExpression,
) -> bool {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::And { .. }
        | psi_checked_trees::CheckedBooleanExpression::Or { .. } => true,
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_contains_short_circuit(operand)
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            checked_boolean_contains_short_circuit(left)
                || checked_boolean_contains_short_circuit(right)
        }
        psi_checked_trees::CheckedBooleanExpression::Constant(_)
        | psi_checked_trees::CheckedBooleanExpression::Parameter { .. }
        | psi_checked_trees::CheckedBooleanExpression::Local { .. }
        | psi_checked_trees::CheckedBooleanExpression::IntegerComparison { .. } => false,
    }
}

fn is_structural_scalar_return_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::Boolean(expression) => {
            is_structural_boolean_return_expression(expression, scalar_parameters, available_locals)
        }
        expression => is_branch_free_structural_integer_expression(
            expression,
            scalar_parameters,
            available_locals,
        ),
    }
}

fn is_structural_boolean_return_expression(
    expression: &psi_checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::Constant(_) => true,
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_structural_boolean_return_expression(operand, scalar_parameters, available_locals)
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right }
        | psi_checked_trees::CheckedBooleanExpression::And { left, right }
        | psi_checked_trees::CheckedBooleanExpression::Or { left, right } => {
            is_structural_boolean_return_expression(left, scalar_parameters, available_locals)
                && is_structural_boolean_return_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        psi_checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
    }
}

fn is_branch_free_structural_integer_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::IntegerLiteral { .. } => true,
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. } => {
            is_branch_free_structural_integer_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        CheckedScalarExpression::Parameter { position, .. } => *position < scalar_parameters,
        CheckedScalarExpression::Local { position, .. } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        CheckedScalarExpression::Boolean(_) => false,
    }
}

fn is_branch_free_structural_scalar_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::Boolean(expression) => {
            is_branch_free_structural_boolean_expression(
                expression,
                scalar_parameters,
                available_locals,
            )
        }
        expression => is_branch_free_structural_integer_expression(
            expression,
            scalar_parameters,
            available_locals,
        ),
    }
}

fn is_branch_free_structural_boolean_expression(
    expression: &psi_checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::Constant(_) => true,
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_branch_free_structural_boolean_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            is_branch_free_structural_boolean_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_boolean_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        psi_checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        psi_checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        psi_checked_trees::CheckedBooleanExpression::And { .. }
        | psi_checked_trees::CheckedBooleanExpression::Or { .. } => false,
    }
}

fn build_structural_unit_control_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedStructuralUnitControlMachinePlan> {
    let states = program.machine_states(machine);
    if states.len() < 2 {
        return None;
    }
    let binders = machine_binders(program, machine);
    let mut signatures = Vec::with_capacity(states.len());
    let mut attachment_type_identity = None;
    for state in states {
        if !is_unit(program, state.return_type)
            || !program.state_contracts(state).is_empty()
            || facts.flow.ownership.permissions.iter().any(|(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.source == PermissionEventSource::StateEntry
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned
            })
        {
            return None;
        }
        let flow = state_flow(facts, machine.symbol, state.symbol)?;
        if !facts
            .service_reaches
            .rows
            .services(flow.service_reach.direct)
            .is_empty()
            || !facts
                .service_reaches
                .rows
                .services(flow.service_reach.transitive)
                .is_empty()
        {
            return None;
        }
        let (attachment, structural_parameters, scalar_parameters) =
            structural_scalar_signature(program, shapes, machine, state, &binders)?;
        let parameters = structural_parameters;
        if parameters.is_empty()
            || parameters.iter().any(|parameter| {
                parameter.is_self
                    || parameter.multiplicity != Multiplicity::Affine
                    || !parameter.qualifications.is_empty()
            })
            || parameters.len() + scalar_parameters.len() != program.state_parameters(state).len()
        {
            return None;
        }
        if attachment_type_identity
            .as_ref()
            .is_some_and(|identity| identity != &attachment)
        {
            return None;
        }
        attachment_type_identity = Some(attachment);
        signatures.push((parameters, scalar_parameters));
    }

    let mut checked_states = Vec::with_capacity(states.len());
    for (state_index, state) in states.iter().enumerate() {
        let (source_parameters, source_scalar_parameters) = &signatures[state_index];
        let statements = program.statement_table.statements(state.statement_nodes);
        let terminator = match statements {
            [] => CheckedStructuralUnitControlTerminatorPlan::ReturnUnit {
                trivial_affine_discard_parameter_positions:
                    super::terminal_cleanup::checked_whole_affine_discard_parameters(
                        program,
                        facts,
                        machine.symbol,
                        state,
                    )?
                    .into_iter()
                    .map(|(_, position)| position)
                    .collect(),
            },
            [StatementNode::Transition(transition)]
                if transition.exit == TransitionExit::Ordinary
                    && transition.guard == TransitionGuardNode::Always
                    && !transition.continuation.is_valid() =>
            {
                let TransitionTargetNode::Named { path, arguments } =
                    program.statement_table.transition_target(transition.target)
                else {
                    return None;
                };
                let target_index = states
                    .iter()
                    .position(|candidate| candidate.symbol == path.symbol)?;
                let (target_parameters, target_scalar_parameters) = &signatures[target_index];
                let arguments = program.statement_table.expression_handles(*arguments);
                if arguments.len() != target_parameters.len() + target_scalar_parameters.len() {
                    return None;
                }
                let mut transferred_sources = BTreeSet::new();
                let transfers = target_parameters
                    .iter()
                    .enumerate()
                    .map(|(target_index, target)| {
                        let argument = arguments.get(target.position as usize)?;
                        let place = super::canonical_place_from_expression_in_state(
                            program,
                            state.symbol,
                            0,
                            *argument,
                        )?;
                        let psi_facts::PlaceRoot::Symbol(root) = place.root else {
                            return None;
                        };
                        if !place.segments.is_empty() {
                            return None;
                        }
                        let source_index = source_parameters.iter().position(|source| {
                            let source = program
                                .state_parameters(state)
                                .get(source.position as usize);
                            source.is_some_and(|source| source.symbol == root)
                        })?;
                        let source = &source_parameters[source_index];
                        if source.type_identity != target.type_identity
                            || source.multiplicity != target.multiplicity
                            || !transferred_sources.insert(source_index)
                        {
                            return None;
                        }
                        Some(CheckedStructuralControlTransferPlan {
                            source_parameter_index: u32::try_from(source_index).ok()?,
                            target_parameter_index: u32::try_from(target_index).ok()?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let scalar_arguments = target_scalar_parameters
                    .iter()
                    .enumerate()
                    .map(|(target_index, target)| {
                        let argument_ordinal = target.source_position;
                        let expression = facts.values.scalar_expressions.expression_at(
                            state.symbol,
                            0,
                            CheckedScalarExpressionRole::TransitionArgument { argument_ordinal },
                        )?;
                        let source_index = match expression {
                            CheckedScalarExpression::Boolean(expression)
                                if target.primitive_type == PrimitiveType::Bool =>
                            {
                                let psi_checked_trees::CheckedBooleanExpression::Parameter {
                                    position,
                                } = expression.as_ref()
                                else {
                                    return None;
                                };
                                *position
                            }
                            CheckedScalarExpression::Parameter {
                                position,
                                primitive_type,
                            } if *primitive_type == target.primitive_type => *position,
                            _ => return None,
                        };
                        if source_scalar_parameters
                            .get(source_index)
                            .is_none_or(|source| source.primitive_type != target.primitive_type)
                        {
                            return None;
                        }
                        Some(CheckedStructuralScalarArgumentPlan {
                            argument_ordinal,
                            source_scalar_parameter_index: u32::try_from(source_index).ok()?,
                            target_scalar_parameter_index: u32::try_from(target_index).ok()?,
                            primitive_type: target.primitive_type,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
                    machine.symbol,
                    state.symbol,
                    0,
                )?;
                if cleanup.target_state != path.symbol {
                    return None;
                }
                let cleanup_sources = cleanup
                    .trivial_affine_discard_parameter_positions
                    .iter()
                    .map(|position| {
                        source_parameters
                            .iter()
                            .position(|parameter| parameter.position == *position)
                    })
                    .collect::<Option<BTreeSet<_>>>()?;
                if !transferred_sources.is_disjoint(&cleanup_sources)
                    || transferred_sources
                        .union(&cleanup_sources)
                        .copied()
                        .collect::<BTreeSet<_>>()
                        != (0..source_parameters.len()).collect::<BTreeSet<_>>()
                {
                    return None;
                }
                CheckedStructuralUnitControlTerminatorPlan::Jump {
                    statement_ordinal: 0,
                    target_state: path.symbol,
                    transfers,
                    scalar_arguments,
                    trivial_affine_discard_parameter_positions: cleanup
                        .trivial_affine_discard_parameter_positions
                        .clone(),
                }
            }
            [
                StatementNode::Transition(when_true),
                StatementNode::Transition(when_false),
            ] if when_true.exit == TransitionExit::Ordinary
                && matches!(when_true.guard, TransitionGuardNode::When(_))
                && when_false.exit == TransitionExit::Ordinary
                && when_false.guard == TransitionGuardNode::Always
                && !when_true.continuation.is_valid()
                && !when_false.continuation.is_valid() =>
            {
                let guard_expression = facts.values.scalar_expressions.expression_at(
                    state.symbol,
                    0,
                    CheckedScalarExpressionRole::Guard,
                )?;
                let guard_scalar_parameter_index = match guard_expression {
                    CheckedScalarExpression::Boolean(expression)
                        if matches!(
                            expression.as_ref(),
                            psi_checked_trees::CheckedBooleanExpression::Parameter { .. }
                        ) =>
                    {
                        let psi_checked_trees::CheckedBooleanExpression::Parameter { position } =
                            expression.as_ref()
                        else {
                            unreachable!()
                        };
                        let parameter = source_scalar_parameters.get(*position)?;
                        (parameter.primitive_type == PrimitiveType::Bool)
                            .then(|| u32::try_from(*position).ok())??
                    }
                    _ => return None,
                };
                let build_successor =
                    |statement_ordinal: u32,
                     transition: &psi_typed_trees::statement::TableTransition|
                     -> Option<CheckedStructuralControlSuccessorPlan> {
                        let TransitionTargetNode::Named { path, arguments } =
                            program.statement_table.transition_target(transition.target)
                        else {
                            return None;
                        };
                        let target_index = states
                            .iter()
                            .position(|candidate| candidate.symbol == path.symbol)?;
                        let (target_parameters, target_scalar_parameters) =
                            &signatures[target_index];
                        let arguments = program.statement_table.expression_handles(*arguments);
                        if arguments.len()
                            != target_parameters.len() + target_scalar_parameters.len()
                        {
                            return None;
                        }
                        let mut transferred_sources = BTreeSet::new();
                        let transfers = target_parameters
                            .iter()
                            .enumerate()
                            .map(|(target_index, target)| {
                                let argument = arguments.get(target.position as usize)?;
                                let place = super::canonical_place_from_expression_in_state(
                                    program,
                                    state.symbol,
                                    usize::try_from(statement_ordinal).ok()?,
                                    *argument,
                                )?;
                                let psi_facts::PlaceRoot::Symbol(root) = place.root else {
                                    return None;
                                };
                                if !place.segments.is_empty() {
                                    return None;
                                }
                                let source_index = source_parameters.iter().position(|source| {
                                    program
                                        .state_parameters(state)
                                        .get(source.position as usize)
                                        .is_some_and(|parameter| parameter.symbol == root)
                                })?;
                                let source = &source_parameters[source_index];
                                if source.type_identity != target.type_identity
                                    || source.multiplicity != target.multiplicity
                                    || !transferred_sources.insert(source_index)
                                {
                                    return None;
                                }
                                Some(CheckedStructuralControlTransferPlan {
                                    source_parameter_index: u32::try_from(source_index).ok()?,
                                    target_parameter_index: u32::try_from(target_index).ok()?,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let scalar_arguments = target_scalar_parameters
                            .iter()
                            .enumerate()
                            .map(|(target_index, target)| {
                                let argument_ordinal = target.source_position;
                                let expression = facts.values.scalar_expressions.expression_at(
                                    state.symbol,
                                    statement_ordinal,
                                    CheckedScalarExpressionRole::TransitionArgument {
                                        argument_ordinal,
                                    },
                                )?;
                                let source_index = match expression {
                                    CheckedScalarExpression::Boolean(expression)
                                        if target.primitive_type == PrimitiveType::Bool =>
                                    {
                                        let psi_checked_trees::CheckedBooleanExpression::Parameter {
                                            position,
                                        } = expression.as_ref()
                                        else {
                                            return None;
                                        };
                                        *position
                                    }
                                    CheckedScalarExpression::Parameter {
                                        position,
                                        primitive_type,
                                    } if *primitive_type == target.primitive_type => *position,
                                    _ => return None,
                                };
                                if source_scalar_parameters
                                    .get(source_index)
                                    .is_none_or(|source| {
                                        source.primitive_type != target.primitive_type
                                    })
                                {
                                    return None;
                                }
                                Some(CheckedStructuralScalarArgumentPlan {
                                    argument_ordinal,
                                    source_scalar_parameter_index: u32::try_from(source_index)
                                        .ok()?,
                                    target_scalar_parameter_index: u32::try_from(target_index)
                                        .ok()?,
                                    primitive_type: target.primitive_type,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
                            machine.symbol,
                            state.symbol,
                            statement_ordinal,
                        )?;
                        if cleanup.target_state != path.symbol {
                            return None;
                        }
                        let cleanup_sources = cleanup
                            .trivial_affine_discard_parameter_positions
                            .iter()
                            .map(|position| {
                                source_parameters
                                    .iter()
                                    .position(|parameter| parameter.position == *position)
                            })
                            .collect::<Option<BTreeSet<_>>>()?;
                        if !transferred_sources.is_disjoint(&cleanup_sources)
                            || transferred_sources
                                .union(&cleanup_sources)
                                .copied()
                                .collect::<BTreeSet<_>>()
                                != (0..source_parameters.len()).collect::<BTreeSet<_>>()
                        {
                            return None;
                        }
                        Some(CheckedStructuralControlSuccessorPlan {
                            statement_ordinal,
                            target_state: path.symbol,
                            transfers,
                            scalar_arguments,
                            trivial_affine_discard_parameter_positions: cleanup
                                .trivial_affine_discard_parameter_positions
                                .clone(),
                        })
                    };
                let when_true = build_successor(0, when_true)?;
                let when_false = build_successor(1, when_false)?;
                if when_true.target_state == when_false.target_state {
                    return None;
                }
                CheckedStructuralUnitControlTerminatorPlan::Conditional {
                    guard_scalar_parameter_index,
                    when_true,
                    when_false,
                }
            }
            _ => return None,
        };
        checked_states.push(CheckedStructuralUnitControlStatePlan {
            state: state.symbol,
            structural_parameters: source_parameters.clone(),
            scalar_parameters: source_scalar_parameters.clone(),
            terminator,
        });
    }
    if checked_states
        .iter()
        .filter(|state| {
            matches!(
                state.terminator,
                CheckedStructuralUnitControlTerminatorPlan::Conditional { .. }
            )
        })
        .count()
        > 2
    {
        return None;
    }
    let mut predecessor_counts = vec![0_usize; checked_states.len()];
    for state in &checked_states {
        let targets = match &state.terminator {
            CheckedStructuralUnitControlTerminatorPlan::ReturnUnit { .. } => Vec::new(),
            CheckedStructuralUnitControlTerminatorPlan::Jump { target_state, .. } => {
                vec![*target_state]
            }
            CheckedStructuralUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target_state, when_false.target_state],
        };
        for target in targets {
            let target_index = checked_states
                .iter()
                .position(|candidate| candidate.state == target)?;
            let count = predecessor_counts.get_mut(target_index)?;
            *count += 1;
            if *count > 2 {
                return None;
            }
        }
    }
    if predecessor_counts[0] != 0
        || predecessor_counts
            .iter()
            .filter(|count| **count == 2)
            .count()
            > 1
    {
        return None;
    }
    Some(CheckedStructuralUnitControlMachinePlan {
        machine: machine.symbol,
        attachment_type_identity: attachment_type_identity?,
        states: checked_states,
    })
}

fn build_boundary_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedUnitBoundaryMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !is_unit(program, state.return_type)
        || !program
            .statement_table
            .statements(state.statement_nodes)
            .is_empty()
    {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
    let domain_requirements = boundary_domain_requirements(
        program,
        facts,
        shapes,
        machine,
        state,
        &structural_parameters,
        &binders,
    )?;
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;

    Some(CheckedUnitBoundaryMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        domain_requirements,
        contract_fingerprint: contract.fingerprint,
        contract_service_reach: contract.service_reach.clone(),
        service_reach: state_flow.service_reach.clone(),
    })
}

fn build_checked_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedUnitEffectMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !is_unit(program, state.return_type) {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders)?;
    if !checked_state_contracts_supported(program, machine, state, &structural_parameters) {
        return None;
    }
    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span_or_empty(state_flow.calls);
    let statements = program.statement_table.statements(state.statement_nodes);
    if calls.len() != statements.len()
        || statements
            .iter()
            .any(|statement| !matches!(statement, StatementNode::Call(_)))
    {
        return None;
    }

    let mut operations = Vec::with_capacity(calls.len() + 1);
    for (statement_index, call) in calls.iter().enumerate() {
        if call.statement_index != statement_index || call.call_ordinal != 0 {
            return None;
        }
        operations.push(build_call_operation(
            program,
            facts,
            machine,
            state,
            &structural_parameters,
            &entry_claims,
            call,
        )?);
    }
    operations.push(CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index: u32::try_from(statements.len()).ok()?,
        trivial_affine_discards: return_unit_affine_discards(
            facts,
            machine.symbol,
            state.symbol,
            &structural_parameters,
            program.state_parameters(state),
            &operations,
        )?,
    });

    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let mut body_qualifications = facts
        .qualifications
        .for_machine(machine.symbol)
        .map(|fact| fact.body_committed.clone())
        .unwrap_or_default();
    body_qualifications.sort_by_key(|domain| domain.0);
    body_qualifications.dedup();

    Some(CheckedUnitEffectMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        entry_claims,
        body_qualifications,
        contract_fingerprint: contract.fingerprint,
        contract_service_reach: contract.service_reach.clone(),
        service_reach: state_flow.service_reach.clone(),
        operations,
    })
}

fn build_call_operation(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    call: &psi_checked_trees::FlowCallFact,
) -> Option<CheckedUnitEffectOperationPlan> {
    let coordinate = CheckedUnitCallCoordinate {
        statement_index: u32::try_from(call.statement_index).ok()?,
        call_ordinal: u32::try_from(call.call_ordinal).ok()?,
    };
    let call_site = crate::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;

    if program
        .symbols
        .builtin_function_symbol(BuiltinFunction::AsmPortOut)
        == Some(call.target_symbol)
    {
        let arguments = crate::call_site_argument_expressions(program, &call_site);
        let [port, value] = arguments else {
            return None;
        };
        return Some(CheckedUnitEffectOperationPlan::PortWrite {
            coordinate,
            port: exact_integer_at(
                facts,
                machine.symbol,
                state.symbol,
                call.statement_index,
                *port,
                PrimitiveType::U16,
            )?
            .try_into()
            .ok()?,
            value: exact_integer_at(
                facts,
                machine.symbol,
                state.symbol,
                call.statement_index,
                *value,
                PrimitiveType::U8,
            )?
            .try_into()
            .ok()?,
            service_reach: call.service_reach.clone(),
        });
    }

    let target_state = crate::find_state(program, call.target_symbol)?;
    let target_machine = program.machines().iter().find(|candidate| {
        program
            .machine_states(candidate)
            .iter()
            .any(|candidate_state| candidate_state.symbol == target_state.symbol)
    })?;
    if !is_unit(program, target_state.return_type) {
        return None;
    }
    let target_contract = facts.contract_plans.for_machine(target_machine.symbol)?;
    let structural_arguments = structural_call_arguments(
        program,
        machine,
        state,
        caller_parameters,
        target_machine,
        target_state,
        &call_site,
        call.receiver_symbol,
    )?;
    let boundary = target_machine.supply_mode.is_boundary_declaration();
    if !boundary && target_machine.supply_mode != MachineSupplyMode::CheckedBody {
        return None;
    }
    let transfers = call_claim_transfers(
        facts,
        machine.symbol,
        state.symbol,
        call,
        caller_parameters,
        entry_claims,
        &structural_arguments,
        if boundary {
            PermissionEventKind::Consume
        } else {
            PermissionEventKind::Transfer
        },
    )?;

    if boundary {
        Some(CheckedUnitEffectOperationPlan::BoundaryCallUnit {
            coordinate,
            target_machine: target_machine.symbol,
            target_state: target_state.symbol,
            target_contract_fingerprint: target_contract.fingerprint,
            service_reach: call.service_reach.clone(),
            structural_arguments,
            completion_receipts: transfers,
        })
    } else {
        Some(CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine: target_machine.symbol,
            target_state: target_state.symbol,
            target_contract_fingerprint: target_contract.fingerprint,
            service_reach: call.service_reach.clone(),
            structural_arguments,
            claim_transfers: transfers,
        })
    }
}

fn structural_call_arguments(
    program: &TypedTrees,
    caller_machine: &psi_typed_trees::machine::Machine,
    caller_state: &psi_typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    target_machine: &psi_typed_trees::machine::Machine,
    target_state: &psi_typed_trees::state::State,
    call_site: &crate::CallSite<'_>,
    receiver_symbol: SymbolHandle,
) -> Option<Vec<CheckedUnitStructuralArgumentPlan>> {
    let source_parameters = program.state_parameters(caller_state);
    let target_parameters = program.state_parameters(target_state);
    let explicit_arguments = crate::call_site_argument_expressions(program, call_site);
    let mut explicit_index = 0usize;
    let mut output = Vec::new();

    for target in target_parameters {
        let source_symbol = if target.is_self {
            if is_reference(program, target.type_reference) {
                continue;
            }
            receiver_symbol
        } else {
            let expression = *explicit_arguments.get(explicit_index)?;
            explicit_index += 1;
            crate::lookup::expression_root_symbol(
                expression,
                &program.expression_table,
                caller_machine.symbol,
            )?
        };
        let source_parameter = source_parameters.iter().find(|parameter| {
            parameter_root_symbol(caller_machine.symbol, parameter) == source_symbol
        })?;
        let source_index = caller_parameters.iter().position(|candidate| {
            candidate.position
                == u32::try_from(
                    source_parameters
                        .iter()
                        .position(|parameter| parameter.symbol == source_parameter.symbol)
                        .unwrap_or(usize::MAX),
                )
                .unwrap_or(u32::MAX)
        })?;
        let source_identity = caller_parameters.get(source_index)?.type_identity.clone();
        let target_identity = if target.is_self {
            attached_data_identity(program, target_machine)?
        } else {
            base_type_identity(program, target.type_reference, &[])?
        };
        if source_identity != target_identity {
            return None;
        }
        output.push(CheckedUnitStructuralArgumentPlan {
            source_parameter_index: u32::try_from(source_index).ok()?,
            type_identity: target_identity,
        });
    }
    if explicit_index != explicit_arguments.len() {
        return None;
    }
    Some(output)
}

fn call_claim_transfers(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &psi_checked_trees::FlowCallFact,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    arguments: &[CheckedUnitStructuralArgumentPlan],
    kind: PermissionEventKind,
) -> Option<Vec<CheckedUnitClaimTransferPlan>> {
    let events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source
                    == PermissionEventSource::Call {
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_symbol: call.target_symbol,
                    }
                && event.kind == kind
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    for (argument_index, argument) in arguments.iter().enumerate() {
        let entries = entry_claims
            .iter()
            .filter(|entry| entry.parameter_index == argument.source_parameter_index)
            .collect::<Vec<_>>();
        if entries.is_empty() {
            if caller_parameters
                .get(argument.source_parameter_index as usize)?
                .multiplicity
                == Multiplicity::Linear
            {
                return None;
            }
            continue;
        }
        for entry in entries {
            let matching = events
                .iter()
                .filter(|event| event.claim_identity == entry.claim_identity)
                .collect::<Vec<_>>();
            if matching.len() != 1 || entry.claim_identity == PermissionClaimIdentity::Unknown {
                return None;
            }
            output.push(CheckedUnitClaimTransferPlan {
                claim_identity: entry.claim_identity,
                argument_index: u32::try_from(argument_index).ok()?,
            });
        }
    }
    if output.len() != events.len() {
        return None;
    }
    Some(output)
}

fn exact_integer_at(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    expression: psi_typed_trees::expression::ExpressionHandle,
    expected_type: PrimitiveType,
) -> Option<u64> {
    let matches = facts
        .values
        .expression_values(expression)
        .filter(|(_, value)| {
            value.origin
                == psi_checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol: machine,
                    state_symbol: state,
                    statement_index,
                    role: psi_checked_trees::CheckedValueStatementRole::CallArgument,
                }
        })
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    let [value] = matches.as_slice() else {
        return None;
    };
    if value.primitive_type != Some(expected_type) {
        return None;
    }
    let range = value.integer_range.as_ref()?;
    (range.minimum == range.maximum)
        .then(|| range.minimum.to_u64())
        .flatten()
}

fn structural_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(String, Vec<CheckedUnitStructuralParameterPlan>)> {
    let parameters = program.state_parameters(state);
    let attached_name = machine.attached_data.as_ref()?;
    let attached = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_name)?;
    let attachment_type_identity = shapes.add_attached_data(attached, binders)?;
    let attachment_multiplicity = attached.properties.multiplicity;
    let mut structural_parameters = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        if parameter.is_const {
            return None;
        }
        if parameter.is_self && is_reference(program, parameter.type_reference) {
            continue;
        }
        if is_reference(program, parameter.type_reference) {
            return None;
        }
        // Typed attached `self` intentionally carries the machine/Self symbol,
        // not the data-definition symbol. Its carrier is the independently
        // resolved attachment above.
        let type_identity = if parameter.is_self {
            attachment_type_identity.clone()
        } else {
            shapes.add_type(parameter.type_reference, binders, &[])?
        };
        let qualifications =
            parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: u32::try_from(position).ok()?,
            is_self: parameter.is_self,
            type_identity,
            multiplicity: if parameter.is_self {
                attachment_multiplicity
            } else {
                crate::checks::type_multiplicity(program, parameter.type_reference)
            },
            qualifications,
        });
    }
    Some((attachment_type_identity, structural_parameters))
}

fn structural_scalar_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(
    String,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    let parameters = program.state_parameters(state);
    let attached_name = machine.attached_data.as_ref()?;
    let attached = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_name)?;
    let attachment_type_identity = shapes.add_attached_data(attached, binders)?;
    let attachment_multiplicity = attached.properties.multiplicity;
    let mut structural_parameters = Vec::new();
    let mut scalar_parameters = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        let source_position = u32::try_from(position).ok()?;
        if let Some(primitive_type) = program.primitive_type_reference(parameter.type_reference) {
            if parameter.is_self || parameter.is_const || parameter.is_mutable {
                return None;
            }
            scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                source_position,
                primitive_type,
            });
            continue;
        }
        if parameter.is_const {
            return None;
        }
        if parameter.is_self && is_reference(program, parameter.type_reference) {
            continue;
        }
        if is_reference(program, parameter.type_reference) {
            return None;
        }
        let type_identity = if parameter.is_self {
            attachment_type_identity.clone()
        } else {
            shapes.add_type(parameter.type_reference, binders, &[])?
        };
        let qualifications =
            parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: source_position,
            is_self: parameter.is_self,
            type_identity,
            multiplicity: if parameter.is_self {
                attachment_multiplicity
            } else {
                crate::checks::type_multiplicity(program, parameter.type_reference)
            },
            qualifications,
        });
    }
    Some((
        attachment_type_identity,
        structural_parameters,
        scalar_parameters,
    ))
}

fn entry_claims(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    source_parameters: &[StateParameter],
) -> Option<Vec<CheckedUnitEntryClaimPlan>> {
    let events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    for (parameter_index, parameter) in structural_parameters.iter().enumerate() {
        if parameter.multiplicity == Multiplicity::Unrestricted {
            continue;
        }
        let source = source_parameters.get(parameter.position as usize)?;
        let expected_root = psi_facts::PlaceRoot::Symbol(parameter_root_symbol(machine, source));
        let matching = events
            .iter()
            .filter(|event| event.root == expected_root)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            if parameter.multiplicity == Multiplicity::Affine {
                continue;
            }
            return None;
        }
        for event in matching {
            if event.claim_identity == PermissionClaimIdentity::Unknown {
                return None;
            }
            let policies = facts
                .carry
                .claim_policies
                .iter()
                .filter(|policy| policy.claim_identity == event.claim_identity)
                .collect::<Vec<_>>();
            let carry = match policies.as_slice() {
                [] => CarryPolicy::STRICT,
                [policy] => policy.effective,
                _ => return None,
            };
            let field_path = facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .iter()
                .map(|segment| match segment {
                    psi_facts::PlaceSegment::Field { symbol } => {
                        terminal_field_identity(program, *symbol)
                    }
                    psi_facts::PlaceSegment::Case { .. }
                    | psi_facts::PlaceSegment::FixedIndex { .. }
                    | psi_facts::PlaceSegment::Index { .. } => None,
                })
                .collect::<Option<Vec<_>>>()?;
            output.push(CheckedUnitEntryClaimPlan {
                claim_identity: event.claim_identity,
                parameter_index: u32::try_from(parameter_index).ok()?,
                field_path,
                carry,
            });
        }
    }
    output.sort_by(|left, right| {
        (left.parameter_index, &left.field_path).cmp(&(right.parameter_index, &right.field_path))
    });
    (output.len() == events.len()).then_some(output)
}

fn return_unit_affine_discards(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    source_parameters: &[StateParameter],
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Vec<u32>> {
    let transferred_parameters = operations
        .iter()
        .flat_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCallUnit {
                structural_arguments,
                ..
            } => structural_arguments
                .iter()
                .map(|argument| argument.source_parameter_index)
                .collect::<Vec<_>>(),
            CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => Vec::new(),
        })
        .collect::<BTreeSet<_>>();
    let events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Affine
                && !event.obligation_live
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty()
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(events.len());
    for event in events {
        let parameter_index = structural_parameters.iter().position(|parameter| {
            source_parameters
                .get(parameter.position as usize)
                .is_some_and(|source| {
                    event.root
                        == psi_facts::PlaceRoot::Symbol(parameter_root_symbol(machine, source))
                })
        })?;
        let parameter = &structural_parameters[parameter_index];
        if parameter.multiplicity != Multiplicity::Affine
            || output.contains(&(parameter_index as u32))
        {
            return None;
        }
        let parameter_index = u32::try_from(parameter_index).ok()?;
        if !transferred_parameters.contains(&parameter_index) {
            output.push(parameter_index);
        }
    }
    Some(output)
}

fn terminal_field_identity(program: &TypedTrees, symbol: SymbolHandle) -> Option<String> {
    program.data_definitions().iter().find_map(|definition| {
        program.data_members(definition).iter().find_map(|member| {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == symbol).then(|| {
                field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned())
            })
        })
    })
}

fn checked_state_contracts_supported(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
) -> bool {
    let source_parameters = program.state_parameters(state);
    program.state_contracts(state).iter().all(|contract| {
        program
            .proof_facts
            .span_or_empty(contract.facts)
            .iter()
            .all(|fact| match (&contract.kind, fact) {
                (SignatureContractKind::Requires, ProofFact::Membership(membership)) => {
                    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                        program,
                        state.symbol,
                        0,
                        membership.value,
                    ) else {
                        return false;
                    };
                    if !place.segments.is_empty() {
                        return false;
                    }
                    let psi_facts::PlaceRoot::Symbol(root) = place.root else {
                        return false;
                    };
                    let Some(position) = source_parameters.iter().position(|parameter| {
                        parameter_root_symbol(machine.symbol, parameter) == root
                            || parameter.symbol == root
                    }) else {
                        return false;
                    };
                    let Some(domain) = program
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == membership.domain_symbol)
                    else {
                        return false;
                    };
                    structural_parameters.iter().any(|parameter| {
                        parameter.position as usize == position
                            && parameter.qualifications.contains(&domain.semantic_id)
                    })
                }
                (SignatureContractKind::Ensures, ProofFact::Expression(expression)) => matches!(
                    program.expression_table.expression(*expression),
                    psi_typed_trees::expression::ExpressionNode::Boolean(true)
                ),
                _ => false,
            })
    })
}

fn boundary_domain_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<CheckedUnitStructuralDomainRequirementPlan>> {
    let source_parameters = program.state_parameters(state);
    let checked_requires = facts
        .proof
        .contract_facts
        .iter()
        .filter(|(_, fact)| {
            fact.kind == ContractProofFactKind::Requires
                && (matches!(
                    fact.owner,
                    ContractProofFactOwner::Machine { machine_symbol }
                        if machine_symbol == machine.symbol
                ) || matches!(
                    fact.owner,
                    ContractProofFactOwner::MachineState { machine_symbol, state_symbol }
                        if machine_symbol == machine.symbol && state_symbol == state.symbol
                ))
        })
        .map(|(_, fact)| fact)
        .collect::<Vec<_>>();
    let authored_requires = program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .map(|contract| contract.facts.count() as usize)
        .sum::<usize>();
    if checked_requires.len() != authored_requires {
        return None;
    }

    let mut output = Vec::new();
    for checked in checked_requires {
        let ProofFact::Membership(membership) = program.proof_facts.get(checked.fact) else {
            return None;
        };
        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            0,
            membership.value,
        )?;
        if !place.segments.is_empty() {
            return None;
        }
        let psi_facts::PlaceRoot::Symbol(root) = place.root else {
            return None;
        };
        let source_position = source_parameters.iter().position(|parameter| {
            parameter_root_symbol(machine.symbol, parameter) == root || parameter.symbol == root
        })?;
        let argument_index = structural_parameters
            .iter()
            .position(|parameter| parameter.position as usize == source_position)?;
        let domain = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == membership.domain_symbol)?;
        if !domain.semantic_id.is_valid() {
            return None;
        }
        shapes.add_domain(domain.semantic_id, domain.target_type, binders)?;
        output.push(CheckedUnitStructuralDomainRequirementPlan {
            argument_index: u32::try_from(argument_index).ok()?,
            domain: domain.semantic_id,
        });
    }
    output.sort_by_key(|requirement| (requirement.argument_index, requirement.domain.0));
    output.dedup();
    Some(output)
}

fn parameter_qualifications(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    mut type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<SemanticDomainId>> {
    let mut output = Vec::new();
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        return None;
                    };
                    if !domain.semantic_id.is_valid() {
                        return None;
                    }
                    let definition = program
                        .domain_definitions()
                        .iter()
                        .find(|definition| definition.symbol == domain.symbol)?;
                    shapes.add_domain(domain.semantic_id, definition.target_type, binders)?;
                    output.push(domain.semantic_id);
                }
                type_reference = *base_type;
            }
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            _ => break,
        }
    }
    output.sort_by_key(|domain| domain.0);
    output.dedup();
    Some(output)
}

fn state_flow<'a>(
    facts: &'a CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<&'a psi_checked_trees::FlowStateFact> {
    facts.flow.control.states.iter().find_map(|(_, candidate)| {
        (candidate.machine_symbol == machine && candidate.state_symbol == state)
            .then_some(candidate)
    })
}

fn machine_binders(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Vec<(SymbolHandle, String)> {
    program
        .machine_type_parameters(machine)
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.symbol, format!("$T{index}")))
        .collect()
}

fn parameter_root_symbol(machine: SymbolHandle, parameter: &StateParameter) -> SymbolHandle {
    if parameter.is_self {
        machine
    } else {
        parameter.symbol
    }
}

fn is_reference(program: &TypedTrees, mut type_reference: TypeReferenceHandle) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { .. } => return true,
            _ => return false,
        }
    }
}

fn is_unit(program: &TypedTrees, mut type_reference: TypeReferenceHandle) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Unit => return true,
            _ => return false,
        }
    }
}

fn base_type_identity(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<String> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => type_reference = *referee,
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                return Some(
                    program
                        .normalized_type_identity_with_binders(type_reference, binders)
                        .into_string(),
                );
            }
            _ => return None,
        }
    }
}

fn attached_data_identity(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<String> {
    let name = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *name)?;
    if !program.data_type_parameters(data).is_empty() {
        return None;
    }
    let path = program.symbols.display_path(data.symbol, "::");
    Some(format!("named({})", normalized_atom("name", &path)))
}

struct ShapeCollector<'program> {
    program: &'program TypedTrees,
    types: BTreeMap<String, CheckedUnitStructuralTypePlan>,
    domains: Vec<CheckedUnitStructuralDomainPlan>,
    in_progress: BTreeSet<String>,
}

impl<'program> ShapeCollector<'program> {
    fn new(program: &'program TypedTrees) -> Self {
        Self {
            program,
            types: BTreeMap::new(),
            domains: Vec::new(),
            in_progress: BTreeSet::new(),
        }
    }

    fn add_domain(
        &mut self,
        domain: SemanticDomainId,
        carrier: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<()> {
        let carrier_type_identity = self.add_type(carrier, binders, &[])?;
        let identity = self.program.semantic_domains.name(domain)?.to_owned();
        let plan = CheckedUnitStructuralDomainPlan {
            domain,
            identity,
            carrier_type_identity,
        };
        if let Some(existing) = self
            .domains
            .iter()
            .find(|existing| existing.domain == domain)
        {
            return (existing == &plan).then_some(());
        }
        self.domains.push(plan);
        Some(())
    }

    fn add_attached_data(
        &mut self,
        data: &psi_typed_trees::data::DataDefinition,
        binders: &[(SymbolHandle, String)],
    ) -> Option<String> {
        if !self.program.data_type_parameters(data).is_empty() {
            // A static attached machine does not carry an instantiated type
            // argument tuple. Generic attached data therefore needs a later
            // explicit checked identity fact rather than guessed binding.
            return None;
        }
        let path = self.program.symbols.display_path(data.symbol, "::");
        let identity = format!("named({})", normalized_atom("name", &path));
        self.add_data_shape(identity, data.clone(), binders, Vec::new())
    }

    fn add_type(
        &mut self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<String> {
        let mut type_reference = type_reference;
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => type_reference = *referee,
                TypeReferenceNode::Named { symbol, .. } => {
                    if let Some((_, replacement)) = substitutions
                        .iter()
                        .rev()
                        .find(|(parameter, _)| parameter == symbol)
                    {
                        type_reference = *replacement;
                        continue;
                    }
                    break;
                }
                _ => break,
            }
        }
        let identity = self
            .program
            .normalized_type_identity_with_binders(type_reference, binders)
            .into_string();
        if self.types.contains_key(&identity) {
            return Some(identity);
        }
        let (data_symbol, arguments) = match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Named { symbol, name }
                if PrimitiveType::from_name(name.as_str()).is_none() =>
            {
                (*symbol, Vec::new())
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => (
                *base_symbol,
                self.program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .to_vec(),
            ),
            _ => return None,
        };
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == data_symbol)?
            .clone();
        let members = self.program.data_members(&data);
        if data.supply_mode != psi_language_semantics::DataSupplyMode::CheckedShape
            || !matches!(
                psi_typed_trees::data::DataDefinition::shape_kind_from_members(members),
                DataShapeKind::Empty | DataShapeKind::Record
            )
        {
            return None;
        }
        let data_parameters = self.program.data_type_parameters(&data);
        if data_parameters.len() != arguments.len() {
            return None;
        }
        let mut local_substitutions = substitutions.to_vec();
        local_substitutions.extend(
            data_parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.symbol, argument)),
        );
        self.add_data_shape(identity, data, binders, local_substitutions)
    }

    fn add_data_shape(
        &mut self,
        identity: String,
        data: psi_typed_trees::data::DataDefinition,
        binders: &[(SymbolHandle, String)],
        substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    ) -> Option<String> {
        if self.types.contains_key(&identity) {
            return Some(identity);
        }
        if !self.in_progress.insert(identity.clone()) {
            return None;
        }
        let members = self.program.data_members(&data).to_vec();
        if data.supply_mode != psi_language_semantics::DataSupplyMode::CheckedShape
            || !matches!(
                psi_typed_trees::data::DataDefinition::shape_kind_from_members(&members),
                DataShapeKind::Empty | DataShapeKind::Record
            )
        {
            self.in_progress.remove(&identity);
            return None;
        }
        let mut fields = Vec::new();
        for member in &members {
            let DataMember::Field(field) = member else {
                self.in_progress.remove(&identity);
                return None;
            };
            let field_type = if field.relevance.is_erased() {
                CheckedUnitStructuralFieldType::Erased {
                    type_identity: self
                        .program
                        .normalized_type_identity_with_binders_and_substitutions(
                            field.type_reference,
                            binders,
                            &substitutions,
                        )
                        .into_string(),
                }
            } else {
                match scalar_type(self.program, field.type_reference, &substitutions) {
                    Some(primitive) => CheckedUnitStructuralFieldType::Scalar(primitive),
                    None => {
                        let Some(nested) =
                            self.add_type(field.type_reference, binders, &substitutions)
                        else {
                            self.in_progress.remove(&identity);
                            return None;
                        };
                        if nested == identity {
                            self.in_progress.remove(&identity);
                            return None;
                        }
                        CheckedUnitStructuralFieldType::Structural {
                            type_identity: nested,
                        }
                    }
                }
            };
            fields.push(CheckedUnitStructuralFieldPlan {
                identity: field
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| field.name.as_str().to_owned()),
                relevance: field.relevance,
                field_type,
            });
        }
        self.types.insert(
            identity.clone(),
            CheckedUnitStructuralTypePlan {
                identity: identity.clone(),
                fields,
            },
        );
        self.in_progress.remove(&identity);
        Some(identity)
    }

    fn retain_transitive(&mut self, roots: &BTreeSet<&str>) {
        let mut retained = roots
            .iter()
            .map(|root| (*root).to_owned())
            .collect::<BTreeSet<_>>();
        loop {
            let old_len = retained.len();
            for identity in retained.clone() {
                let Some(plan) = self.types.get(&identity) else {
                    continue;
                };
                for field in &plan.fields {
                    if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                        &field.field_type
                    {
                        retained.insert(type_identity.clone());
                    }
                }
            }
            if retained.len() == old_len {
                break;
            }
        }
        self.types.retain(|identity, _| retained.contains(identity));
        self.domains
            .retain(|domain| retained.contains(&domain.carrier_type_identity));
    }
}

fn scalar_type(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<PrimitiveType> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, name } => {
                if let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                {
                    type_reference = *replacement;
                    continue;
                }
                return PrimitiveType::from_name(name.as_str());
            }
            _ => return None,
        }
    }
}

fn normalized_atom(tag: &str, value: &str) -> String {
    let mut output = String::with_capacity(tag.len() + value.len() + 2);
    output.push_str(tag);
    output.push('(');
    for character in value.chars() {
        if matches!(character, '\\' | '(' | ')' | ',') {
            output.push('\\');
        }
        output.push(character);
    }
    output.push(')');
    output
}
