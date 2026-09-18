//! Preparing a scalar-graph machine under its contract mode.

use super::primitive_locals;
use crate::emission::expression_validation::validate_direct_parameter_types;
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::expression_preparation::qualifications::PreparedScalarQualifications;
use crate::scalar_graph::scalar_computations as computations;
use crate::scalar_graph::scalar_graph_lowering::call_lowering::lower_scalar_graph_successor;
use crate::scalar_graph::scalar_graph_lowering::contract_lowering::{
    closed_scalar_contract_plan, exact_direct_result_float_meaning_reflexivity_contract,
    lower_content_evidence, validate_closed_scalar_contract, validate_empty_scalar_contract_source,
};
use crate::scalar_graph::scalar_graph_lowering::graph_validation::validate_scalar_graph;
use crate::scalar_graph::scalar_graph_lowering::known_evaluation::evaluate_known_scalar_graph;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
    PreparedScalarContract, PreparedScalarMachine,
};
use crate::scalar_graph::scalar_graph_lowering::{
    bindings, branch_destinations, cycles, guards, structural_values,
};
use crate::scalar_graph::{
    CheckedScalarBranchDestination, CheckedScalarExpressionRole, CheckedScalarMachineGraph,
    CheckedScalarStateTerminator, CheckedTrees, ClosedScalarContractValue, LoweringError,
    Multiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    lower_checked_crash_exit, scalar_carriers, unsupported,
};

pub(crate) fn prepare_scalar_graph_machine(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    machine: symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<PreparedScalarMachine, LoweringError> {
    prepare_scalar_graph_machine_with_contract_mode(
        checked,
        qualifications,
        machine,
        graph,
        ScalarContractMode::ClosedRuntimeValue,
        &[],
        &[],
        &[],
        &mut 1,
    )
}

pub(crate) fn prepare_standalone_scalar_graph_machine(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<PreparedScalarMachine, LoweringError> {
    let qualifications = PreparedScalarQualifications::prepare(checked, &[machine])?;
    prepare_scalar_graph_machine_with_contract_mode(
        checked,
        &qualifications,
        machine,
        graph,
        ScalarContractMode::StandaloneProofOnlyFloatResult,
        &[],
        &[],
        &[],
        &mut 1,
    )
}

pub(crate) fn prepare_scalar_graph_in_namespace(
    checked: &CheckedTrees,
    graph: &CheckedScalarMachineGraph,
    parameters: &[StructuralParameterDeclaration],
    primitive_locals: &[primitive_locals::PrimitiveLocal],
    structural_types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<PreparedScalarMachine, LoweringError> {
    let qualifications = PreparedScalarQualifications::prepare(checked, &[graph.machine])?;
    if !qualifications.catalog().domains.is_empty() {
        return unsupported(
            "embedded scalar qualifications require the enclosing catalog namespace",
        );
    }
    prepare_scalar_graph_machine_with_contract_mode(
        checked,
        &qualifications,
        graph.machine,
        graph,
        ScalarContractMode::ClosedRuntimeValue,
        parameters,
        primitive_locals,
        structural_types,
        next_place,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarContractMode {
    ClosedRuntimeValue,
    StandaloneProofOnlyFloatResult,
}

fn prepare_scalar_graph_machine_with_contract_mode(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    machine: symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
    contract_mode: ScalarContractMode,
    structural_parameters: &[StructuralParameterDeclaration],
    primitive_locals: &[primitive_locals::PrimitiveLocal],
    structural_types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<PreparedScalarMachine, LoweringError> {
    let states = &graph.states;
    let source_machine = checked
        .machines()
        .iter()
        .find(|source| source.symbol == machine)
        .ok_or(LoweringError::Unsupported(
            "scalar graph has no source machine",
        ))?;
    for retained in states {
        let source = checked
            .machine_states(source_machine)
            .iter()
            .find(|source| source.symbol == retained.state)
            .ok_or(LoweringError::Unsupported(
                "scalar graph has no exact source state",
            ))?;
        // Qualified signatures carry these exact membership requirements.
        // Reconstruct the source obligation; a producer's missing contract row
        // cannot authorize erasing a predicate, route, or unrelated state clause.
        if !validation::scalar_state_contracts_are_qualifications(&checked.typed, source) {
            return unsupported("scalar state contract is not carried by its qualified signature");
        }
    }
    let entry_state = states.first().ok_or(LoweringError::Unsupported(
        "checked scalar control plan must contain an entry state",
    ))?;
    let (_, result_type) = qualifications.scalar_state_types(checked, entry_state.state)?;
    if entry_state.structural_parameters.len() != structural_parameters.len()
        || ((!structural_parameters.is_empty() || !primitive_locals.is_empty())
            && states.len() != 1)
        || states
            .iter()
            .map(|state| state.primitive_locals.len())
            .sum::<usize>()
            != primitive_locals.len()
    {
        return unsupported(
            "scalar graph requires its exact structural entry namespace; structural state forwarding remains unsupported",
        );
    }
    let loop_plan = cycles::prepare(checked, graph, structural_parameters, next_place)?;
    let structural_parameters = loop_plan
        .as_ref()
        .map_or(structural_parameters, |plan| plan.parameters.as_slice());
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry_state.state)?;
    let return_sink = states
        .iter()
        .any(|state| match &state.terminator {
            CheckedScalarStateTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                matches!(when_true, CheckedScalarBranchDestination::Return { .. })
                    || matches!(when_false, CheckedScalarBranchDestination::Return { .. })
            }
            CheckedScalarStateTerminator::Return { statement_ordinal } => {
                checked.facts.values.scalar_computations.roots.iter().any(|(_, root)| {
                    root.state == state.state
                        && root.statement_ordinal == *statement_ordinal
                        && root.role == CheckedScalarExpressionRole::Return
                }) || state.unit_operations.iter().any(|operation| {
                    matches!(operation, checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } if result.multiplicity == Multiplicity::Affine)
                })
            }
            _ => false,
        })
        .then_some(states.len());
    let lowered_state_count = states.len() + usize::from(return_sink.is_some());
    let mut lowered_states = Vec::with_capacity(lowered_state_count);
    let arrays = computations::arrays::prepare(checked, machine, structural_types, next_place)?;
    let cases = computations::cases::prepare(checked, machine, structural_types, next_place)?;
    let fields = computations::fields::prepare(checked, machine, structural_types)?;
    let mut computations =
        computations::Expansion::new(checked, qualifications, machine, lowered_state_count)
            .with_arrays(&arrays)
            .with_cases(&cases)
            .with_fields(&fields);

    for state in states {
        let (parameter_types, state_result_type) =
            qualifications.scalar_state_types(checked, state.state)?;
        if state_result_type != result_type
            || state_result_type.scalar_type != terminal_scalar_type(state.result_type)?
            || parameter_types.len() != state.parameter_types.len()
        {
            return unsupported("scalar graph state result types must match exactly");
        }
        for (actual, retained) in parameter_types.iter().zip(&state.parameter_types) {
            if actual.scalar_type != terminal_scalar_type(*retained)? {
                return unsupported(
                    "scalar graph parameter carrier disagrees with its declaration",
                );
            }
        }
        let prepared = bindings::prepare(
            checked,
            qualifications,
            machine,
            state,
            parameter_types,
            structural_parameters,
            primitive_locals,
            structural_types,
            next_place,
        )?;
        let value_types = &prepared.value_types;
        let scalar_bindings = &prepared.scalar_bindings;
        let terminator = match &state.terminator {
            CheckedScalarStateTerminator::Return { statement_ordinal } => {
                let completed_target = return_sink
                    .map(|target| {
                        structural_values::exit_target(
                            checked,
                            state.state,
                            scalar_bindings,
                            &[result_type],
                            &mut Vec::new(),
                            target,
                            &mut computations,
                            structural_types,
                            next_place,
                        )
                    })
                    .transpose()?;
                let computed_entry = if let Some(target) = completed_target {
                    computations.return_value(
                        state.state,
                        *statement_ordinal,
                        CheckedScalarExpressionRole::Return,
                        scalar_bindings,
                        value_types,
                        result_type,
                        target,
                    )?
                } else {
                    None
                };
                if let Some(target) = computed_entry {
                    LoweredScalarBranchTerminator::Jump {
                        trivial_affine_discards: Vec::new(),
                        target,
                        arguments: computations::parameters(value_types),
                        structural_arguments: Vec::new(),
                    }
                } else {
                    let expression = scalar_bindings.expression_at(
                        checked,
                        state.state,
                        *statement_ordinal,
                        CheckedScalarExpressionRole::Return,
                    )?;
                    if expression.value_type(value_types)? != result_type {
                        return unsupported(
                            "checked scalar return type must match the machine result",
                        );
                    }
                    validate_direct_parameter_types(&expression, &scalar_carriers(value_types))?;
                    if let Some(target) = completed_target {
                        LoweredScalarBranchTerminator::Jump {
                            target,
                            arguments: vec![expression],
                            structural_arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        }
                    } else {
                        LoweredScalarBranchTerminator::Return { expression }
                    }
                }
            }
            CheckedScalarStateTerminator::Crash { statement_ordinal } => {
                LoweredScalarBranchTerminator::Crash(lower_checked_crash_exit(
                    checked,
                    machine,
                    state.state,
                    *statement_ordinal,
                    &identity_reshuffles.source_claims,
                )?)
            }
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal,
                when_true,
                when_false,
            } => {
                branch_destinations::validate_coordinates(
                    *guard_statement_ordinal,
                    when_true,
                    when_false,
                )?;
                let (when_true_target, when_true_arguments) =
                    branch_destinations::lower_destination(
                        checked,
                        qualifications,
                        machine,
                        &identity_reshuffles.source_claims,
                        states,
                        state.state,
                        value_types,
                        when_true,
                        scalar_bindings,
                        result_type,
                        return_sink,
                        &mut computations,
                        structural_types,
                        next_place,
                    )?;
                let (when_false_target, when_false_arguments) =
                    branch_destinations::lower_destination(
                        checked,
                        qualifications,
                        machine,
                        &identity_reshuffles.source_claims,
                        states,
                        state.state,
                        value_types,
                        when_false,
                        scalar_bindings,
                        result_type,
                        return_sink,
                        &mut computations,
                        structural_types,
                        next_place,
                    )?;
                guards::lower(
                    checked,
                    state.state,
                    *guard_statement_ordinal,
                    scalar_bindings,
                    value_types,
                    (when_true_target, when_true_arguments),
                    (when_false_target, when_false_arguments),
                    when_false,
                    &mut computations,
                )?
            }
            CheckedScalarStateTerminator::Guarded { .. } => {
                return unsupported("ordered scalar exits require an ordinary completion body");
            }
            CheckedScalarStateTerminator::Jump(successor) => {
                if successor.is_continuation {
                    return unsupported(
                        "an unconditional scalar jump cannot select continuation arguments",
                    );
                }
                let (target, arguments) = lower_scalar_graph_successor(
                    checked,
                    qualifications,
                    states,
                    state.state,
                    value_types,
                    successor,
                    scalar_bindings,
                    &mut computations,
                    structural_types,
                    next_place,
                )?;
                LoweredScalarBranchTerminator::Jump {
                    trivial_affine_discards: Vec::new(),
                    target,
                    arguments,
                    structural_arguments: Vec::new(),
                }
            }
        };
        lowered_states.push(prepared.finish(state.state, terminator, &mut computations)?);
    }

    if return_sink.is_some() {
        // Return expressions are arguments to this private identity block.
        // Existing conditional argument lowering evaluates them only in the
        // selected arm; no source state or executable value is manufactured.
        lowered_states.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            parameter_types: vec![result_type],
            bindings: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Return {
                expression: LoweredDirectExpression::Parameter {
                    position: 0,
                    scalar_type: result_type.scalar_type,
                },
            },
        });
    }

    lowered_states.extend(computations.finish());
    let successors = lowered_states
        .iter()
        .map(|state| match &state.terminator {
            LoweredScalarBranchTerminator::Jump { target, .. }
            | LoweredScalarBranchTerminator::Qualify { target, .. } => vec![*target],
            LoweredScalarBranchTerminator::Conditional {
                when_true_target,
                when_false_target,
                ..
            } => vec![*when_true_target, *when_false_target],
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut indegree = vec![0usize; lowered_states.len()];
    for target in successors.iter().flatten() {
        let Some(degree) = indegree.get_mut(*target) else {
            return unsupported("scalar computation successor is outside its graph");
        };
        *degree += 1;
    }
    if (indegree[0] != 0 && loop_plan.is_none()) || indegree[1..].contains(&0) {
        return unsupported(
            "scalar graph control must be rooted at the machine entry and reach every state",
        );
    }
    let mut visited = vec![false; lowered_states.len()];
    let mut active = vec![false; lowered_states.len()];
    validate_scalar_graph(
        0,
        &successors,
        &mut visited,
        &mut active,
        loop_plan.is_some(),
    )?;
    if visited.iter().any(|visited| !*visited) {
        return unsupported("scalar graph control contains an unreachable state");
    }

    let has_crash = lowered_states.iter().any(|state| {
        matches!(&state.terminator, LoweredScalarBranchTerminator::Crash(_))
            || state.bindings.iter().any(|binding| {
                matches!(binding, LoweredScalarBinding::DirectCall(call)
                        if !call.crash_continuations.is_empty())
            })
            || state.structural_effects.iter().any(|effect| {
                matches!(effect, LoweredScalarEffect::CallUnit(call)
                        if !call.crash_routes.is_empty())
            })
    });
    let has_return = lowered_states.iter().any(|state| {
        matches!(
            &state.terminator,
            LoweredScalarBranchTerminator::Return { .. }
        )
    });
    let expected_value = if loop_plan.is_some() {
        None
    } else {
        evaluate_known_scalar_graph(&lowered_states)
    };
    let plan = closed_scalar_contract_plan(checked, machine)?;
    // Rejoin every authored range against its retained evidence before any
    // contract shape is selected. Requires-tail `FloatRange` clauses are
    // discharged by the floating entry roster, never by the proposition tail.
    crate::unit::runtime_requirements::validate_graph_parameter_ranges(checked, machine, plan)?;
    let requires = crate::scalar_graph::scalar_contracts::covered_requires(plan)?;
    let has_predicates = requires
        .iter()
        .chain(plan.ensures())
        .any(|clause| matches!(clause, Some(ClosedScalarContractValue::Predicate(_))));
    let has_entry_ranges = plan
        .float_entry_ranges()
        .is_some_and(|ranges| !ranges.is_empty());
    // A helper remains a real callee when embedded in another execution plan.
    // Its checked call identity does not discharge requirements or establish
    // guarantees. Retain the same contract regardless of closure-root position.
    let contract = if plan.requires().is_empty()
        && plan.ensures().is_empty()
        && !plan.has_outcome_specific_clauses()
    {
        validate_empty_scalar_contract_source(checked, machine)?;
        PreparedScalarContract::Empty
    } else if has_return {
        if contract_mode == ScalarContractMode::StandaloneProofOnlyFloatResult
            && exact_direct_result_float_meaning_reflexivity_contract(
                checked,
                machine,
                result_type.scalar_type,
                has_crash,
            )
        {
            PreparedScalarContract::Empty
        } else if has_predicates || has_entry_ranges {
            // Requires `FloatRange` rows are the validated floating range
            // clauses; only ensures has no separate evidence channel.
            if plan.has_outcome_specific_clauses() || plan.ensures().iter().any(Option::is_none) {
                return unsupported("scalar contract contains an unsupported clause");
            }
            PreparedScalarContract::Predicates(plan.clone())
        } else {
            PreparedScalarContract::ClosedLiteral(validate_closed_scalar_contract(
                checked,
                machine,
                result_type.scalar_type,
                expected_value,
                // A published crash clause is a ceiling, not a requirement
                // that the body retain a reachable crash. Checked selection
                // can eliminate every crashing RHS while preserving that API.
                has_crash || closed_scalar_contract_plan(checked, machine)?.has_crash_clauses(),
            )?)
        }
    } else {
        if plan.has_outcome_specific_clauses() || plan.ensures().iter().any(Option::is_none) {
            return unsupported("all-crash scalar contract contains an unsupported clause");
        }
        if plan.requires().is_empty() && plan.ensures().is_empty() {
            PreparedScalarContract::Empty
        } else {
            // Entry requirements still constrain a call that never returns.
            // Retain checked guarantees too; ordinary proof reconstruction,
            // not the absence of a return edge here, decides their evidence.
            PreparedScalarContract::Predicates(plan.clone())
        }
    };
    Ok(PreparedScalarMachine {
        source_machine: machine,
        scalar_qualifications: qualifications.catalog().clone(),
        states: lowered_states,
        result_type,
        contract,
        // The emitted contract ceiling is the machine's effective crash
        // routes, not the authored slice alone: a private scalar-graph machine
        // with an inferred contract still owes its callers the union of its
        // retained body evidence, or its own Trap continuations and crash
        // terminators would be uncovered.
        crash_routes: crate::unit::effective_crash_routes(checked, machine)?,
        identity_reshuffles,
        partition_compositions,
        loop_plan,
    })
}
