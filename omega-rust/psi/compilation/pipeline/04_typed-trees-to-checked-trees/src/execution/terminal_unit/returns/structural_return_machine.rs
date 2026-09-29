//! The structural return machine and exact parameter qualifications.

use crate::execution::terminal_unit::calls::{
    build_unit_trivial_affine_locals, entry_claims, structural_signature,
};
use crate::execution::terminal_unit::types::{
    ShapeCollector, machine_binders, parameter_qualifications, projected_parameter_qualifications,
    return_unit_affine_discards,
};
use crate::execution::terminal_unit::{
    CarryPolicy, CheckFacts, CheckedStructuralResultPlan, CheckedStructuralReturnMachinePlan,
    ExpressionNode, Multiplicity, ProofFact, SemanticDomainId, SignatureContractKind,
    StateParameter, StatementNode, TypedTrees,
};

pub(crate) fn build_structural_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedStructuralReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    let (return_statement, local_statements) = statements.split_last()?;
    let StatementNode::Expression(return_expression) = return_statement else {
        return None;
    };
    if !local_statements
        .iter()
        .all(|statement| matches!(statement, StatementNode::LocalData(_)))
    {
        return None;
    }
    let return_expression = *return_expression;
    if !program.machine_contracts(machine).is_empty() {
        return None;
    }
    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders, false)?;
    // The body holds only these locals and a returned name, so no call can
    // take a local's transfer: each one settles by its exit drop.
    let trivial_affine_locals = build_unit_trivial_affine_locals(
        program,
        facts,
        shapes,
        machine,
        state,
        &binders,
        local_statements,
    )?
    .into_iter()
    .map(|(plan, _)| plan)
    .collect::<Vec<_>>();
    let input = structural_parameters.first()?;
    if input.multiplicity != Multiplicity::Linear
        || input.is_self
        || structural_parameters
            .iter()
            .skip(1)
            .any(|discarded| discarded.multiplicity != Multiplicity::Affine || discarded.is_self)
    {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let source_parameter = source_parameters.get(input.position as usize)?;
    let ExpressionNode::Name(path) = program.expression_table.expression(return_expression) else {
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
        || !entry_claim.path.is_empty()
        || entry_claim.carry != CarryPolicy::STRICT
    {
        return None;
    }
    let (trivial_affine_discards, residual_affine_discards, _) = return_unit_affine_discards(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        source_parameters,
        &[],
        &trivial_affine_locals
            .iter()
            .filter_map(|plan| {
                local_statements
                    .get(plan.declaration_ordinal as usize)
                    .and_then(|statement| match statement {
                        StatementNode::LocalData(local) => Some(local.symbol),
                        _ => None,
                    })
            })
            .collect::<Vec<_>>(),
        &shapes.types,
    )?;
    let expected_discards = (1..structural_parameters.len())
        .rev()
        .map(|position| u32::try_from(position).ok())
        .collect::<Option<Vec<_>>>()?;
    if !residual_affine_discards.is_empty() || trivial_affine_discards != expected_discards {
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
    let checked_trees::FlowClaimOutcomeSource::Input {
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
            projected_qualifications: projected_parameter_qualifications(
                program,
                shapes,
                state.return_type,
                &binders,
            )?,
        },
        trivial_affine_local_discard_ordinals: trivial_affine_locals
            .iter()
            .rev()
            .map(|local| local.declaration_ordinal)
            .collect(),
        trivial_affine_locals,
        entry_claim: entry_claim.clone(),
        trivial_affine_discards: expected_discards,
        transferred_claim: entry_claim.claim_identity,
    })
}

pub(crate) fn state_contracts_are_exact_parameter_qualifications(
    program: &TypedTrees,
    state: &typed_trees::state::State,
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
