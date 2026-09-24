//! Preparing a scalar-graph machine under its contract mode.

use super::primitive_locals;
use crate::emission::expression_validation::validate_direct_parameter_types;
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::expression_preparation::qualifications::PreparedScalarQualifications;
use crate::expression_preparation::source_custody;
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

/// Prepare the selected root of a scalar call closure. Its result is the
/// artifact's direct result, so the one proof-only `FloatMeaning`
/// reflexivity `ensures` (see
/// `exact_direct_result_float_meaning_reflexivity_contract`) has no runtime
/// consumer there and contributes no `MachineContract` value. A callee's
/// result feeds a caller instead, so callees keep the closed runtime mode,
/// which rejects that clause rather than erase a guarantee a caller could
/// cite.
pub(crate) fn prepare_scalar_graph_root(
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
        ScalarContractMode::RootDirectResult,
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
    RootDirectResult,
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
    if !checked
        .machines()
        .iter()
        .any(|source| source.symbol == machine)
    {
        return Err(LoweringError::Unsupported(
            "scalar graph has no source machine",
        ));
    }
    for retained in states {
        // A fused graph retains a sibling machine's states beside its own;
        // the exact authored state resolves through its owning machine.
        let (_, source) = source_custody::authored_state(checked, retained.state)
            .map_err(|_| LoweringError::Unsupported("scalar graph has no exact source state"))?;
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
    // Structural formals belong to one body (`terminal_scalar::
    // build_machine_graph`): only a graph of one state carries them, on the
    // entry roster that is the machine signature. A graph of several states,
    // a multi-state machine or tail-fused bodies, carries scalar formals
    // only; a multi-state machine whose non-entry states would bind formals
    // from their incoming edges belongs to the Unit state graph.
    if entry_state.structural_parameters.len() != structural_parameters.len()
        || (states.len() != 1
            && states
                .iter()
                .any(|state| !state.structural_parameters.is_empty()))
        || (!primitive_locals.is_empty() && states.len() != 1)
        || states
            .iter()
            .map(|state| state.primitive_locals.len())
            .sum::<usize>()
            != primitive_locals.len()
    {
        return unsupported(
            "scalar graph requires its exact structural entry namespace; a multi-state graph carries scalar formals only",
        );
    }
    let loop_plan = cycles::prepare(
        checked,
        graph,
        structural_parameters,
        structural_types,
        next_place,
    )?;
    // A re-entered body rebinds its roster as the loop header's parameters
    // (`cycles::prepare`), which checks each back-edge transfer itself.
    let structural_parameters = loop_plan
        .as_ref()
        .map_or(structural_parameters, |plan| plan.parameters.as_slice());
    let entry_namespace = entry_state
        .structural_parameters
        .iter()
        .zip(structural_parameters)
        .map(|(source, declaration)| (source.position, declaration.clone()))
        .collect::<Vec<_>>();
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

    for (state_index, state) in states.iter().enumerate() {
        let state_namespace: &[(u32, StructuralParameterDeclaration)] = if state_index == 0 {
            &entry_namespace
        } else {
            &[]
        };
        computations.refresh_proof_scope(&state.erased_proof_parameters);
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
        let erased_formal_types = qualifications.scalar_state_erased_types(checked, state.state)?;
        let prepared = bindings::prepare(
            checked,
            qualifications,
            state,
            parameter_types,
            erased_formal_types,
            &state.erased_proof_parameters,
            state_namespace,
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
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
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
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: Vec::new(),
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
                let (
                    when_true_target,
                    when_true_arguments,
                    when_true_erased_arguments,
                    when_true_erased_proof_arguments,
                ) = branch_destinations::lower_destination(
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
                let (
                    when_false_target,
                    when_false_arguments,
                    when_false_erased_arguments,
                    when_false_erased_proof_arguments,
                ) = branch_destinations::lower_destination(
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
                let lowered = guards::lower(
                    checked,
                    state.state,
                    *guard_statement_ordinal,
                    scalar_bindings,
                    value_types,
                    (
                        when_true_target,
                        when_true_arguments,
                        when_true_erased_arguments,
                        when_true_erased_proof_arguments,
                    ),
                    (
                        when_false_target,
                        when_false_arguments,
                        when_false_erased_arguments,
                        when_false_erased_proof_arguments,
                    ),
                    when_false,
                    &mut computations,
                )?;
                match lowered {
                    LoweredScalarBranchTerminator::Conditional {
                        condition:
                            crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression::StructuralCaseMembership {
                                source,
                                path,
                                case,
                            },
                        when_true_target,
                        when_true_arguments,
                        when_true_erased_arguments,
                        when_true_erased_proof_arguments,
                        when_false_target,
                        when_false_arguments,
                        when_false_erased_arguments,
                        when_false_erased_proof_arguments,
                    } if path.is_empty()
                        && crate::emission::case_payload_dispatch::direct_case_reads(
                            &when_true_arguments,
                            source,
                            case,
                        ) =>
                    {
                        case_dispatch_terminator(
                            source,
                            case,
                            state_namespace,
                            structural_types,
                            value_types,
                            (
                                when_true_target,
                                when_true_arguments,
                                when_true_erased_arguments,
                                when_true_erased_proof_arguments,
                            ),
                            (
                                when_false_target,
                                when_false_arguments,
                                when_false_erased_arguments,
                                when_false_erased_proof_arguments,
                            ),
                        )?
                    }
                    other => other,
                }
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
                let (target, arguments, erased_arguments, erased_proof_arguments) =
                    lower_scalar_graph_successor(
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
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments: Vec::new(),
                }
            }
        };
        // The entry's roster lives on the machine signature, not the block:
        // an entry block with a nonempty roster is reserved for owned loop
        // forwarding custody.
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
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
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
            }
            | LoweredScalarBranchTerminator::CaseDispatch {
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
    // Rejoin each authored `FloatMeaning` equality clause to the checked
    // equality row the float-meaning binding already minted for that exact
    // `==` expression. The clause's terminal `Atom` cite is only definable
    // against that checked row — an unrecognized clause fails instead of
    // erasing toward `Truth` or `Empty`.
    let mut resolved_plan;
    let plan = if plan.ensures().iter().flatten().any(|clause| {
        matches!(
            clause,
            ClosedScalarContractValue::FloatMeaningEquality { .. }
        )
    }) {
        resolved_plan = plan.clone();
        resolved_plan
            .resolve_float_meaning_equalities(|expression| {
                checked
                    .facts
                    .proof
                    .float_meaning_equalities
                    .iter()
                    .find(|equality| {
                        equality.use_site.is_none() && equality.source_expression == expression
                    })
                    .map(|equality| equality.id)
            })
            .map_err(|_| {
                LoweringError::Unsupported(
                    "scalar contract float-meaning equality lost its checked equality row",
                )
            })?;
        &resolved_plan
    } else {
        plan
    };
    let requires = crate::scalar_graph::scalar_contracts::covered_requires(plan)?;
    let has_predicates = requires.iter().chain(plan.ensures()).any(|clause| {
        matches!(
            clause,
            Some(ClosedScalarContractValue::Predicate(_))
                | Some(ClosedScalarContractValue::FloatMeaningEquality { .. })
        )
    });
    let has_entry_ranges = plan
        .float_entry_ranges()
        .is_some_and(|ranges| !ranges.is_empty())
        || plan
            .integer_entry_ranges()
            .is_some_and(|ranges| !ranges.is_empty());
    // A helper remains a real callee when embedded in another execution plan.
    // Its checked call identity does not discharge requirements or establish
    // guarantees. Retain the same contract regardless of closure-root position;
    // the root's proof-only float reflexivity clause is the one exception
    // (`prepare_scalar_graph_root`).
    let contract = if plan.requires().is_empty()
        && plan.ensures().is_empty()
        && !plan.has_outcome_specific_clauses()
    {
        validate_empty_scalar_contract_source(checked, machine)?;
        PreparedScalarContract::Empty
    } else if has_return {
        if contract_mode == ScalarContractMode::RootDirectResult
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
        state_symbols: states.iter().map(|state| state.state).collect(),
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

/// A union match over a structural operand reaches scalar lowering as a
/// membership conditional whose selected arm reads bound payload fields.
/// Deferred payload reads only resolve once the case's payloads bind as
/// block parameters, so that conditional becomes a structural dispatch: the
/// selected edge carries the scalar payload fields to a dedicated branch
/// block and both outcomes stage through continuations, matching
/// `Terminator::StructuralCase`.
fn case_dispatch_terminator(
    source: semantic_vocabulary::PlaceId,
    selected: semantic_vocabulary::StructuralCaseId,
    state_namespace: &[(u32, StructuralParameterDeclaration)],
    structural_types: &[StructuralTypeDeclaration],
    value_types: &[semantic_vocabulary::QualifiedScalarType],
    when_true: (
        usize,
        Vec<LoweredDirectExpression>,
        Vec<LoweredDirectExpression>,
        Vec<crate::scalar_graph::scalar_contracts::LoweredProofTerm>,
    ),
    when_false: (
        usize,
        Vec<LoweredDirectExpression>,
        Vec<LoweredDirectExpression>,
        Vec<crate::scalar_graph::scalar_contracts::LoweredProofTerm>,
    ),
) -> Result<LoweredScalarBranchTerminator, LoweringError> {
    let Some((_, parameter)) = state_namespace
        .iter()
        .find(|(_, declaration)| declaration.place == source)
    else {
        return unsupported("case dispatch lost the matched operand's structural parameter");
    };
    let cases = match structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)
        .map(|declaration| &declaration.shape)
    {
        Some(terminal_psi::StructuralTypeShape::Sum { cases })
        | Some(terminal_psi::StructuralTypeShape::Mixed { cases, .. }) => cases.as_slice(),
        _ => {
            return unsupported("case dispatch lost the matched operand's declared cases");
        }
    };
    let selected_case = cases
        .iter()
        .find(|declared| declared.id == selected)
        .ok_or(LoweringError::Unsupported(
            "case dispatch selects a case outside its root's sum",
        ))?;
    let mut payloads = Vec::new();
    let mut bound = Vec::new();
    for field in &selected_case.fields {
        let scalar_type = match field.field_type {
            terminal_psi::StructuralFieldType::Scalar(scalar) => scalar,
            terminal_psi::StructuralFieldType::BoundedInteger(integer) => {
                semantic_vocabulary::ScalarType::Integer(integer.integer_type())
            }
            _ => continue,
        };
        if field.relevance.is_erased() {
            continue;
        }
        // Payload slots append after the state's completed value namespace,
        // where the emitted branch block declares them as parameters.
        bound.push(
            crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload {
                source,
                case: selected,
                field: field.id,
                position: value_types.len() + payloads.len(),
            },
        );
        payloads.push((
            field.id,
            semantic_vocabulary::QualifiedScalarType::from(scalar_type),
        ));
    }
    let (
        when_true_target,
        when_true_arguments,
        when_true_erased_arguments,
        when_true_erased_proof_arguments,
    ) = when_true;
    let (
        when_false_target,
        when_false_arguments,
        when_false_erased_arguments,
        when_false_erased_proof_arguments,
    ) = when_false;
    let when_true_arguments: Vec<LoweredDirectExpression> = when_true_arguments
        .into_iter()
        .map(|argument| crate::emission::case_payload_dispatch::substitute_direct(argument, &bound))
        .collect();
    // Every deferred case read must land on a bound scalar payload slot; a
    // read into a non-scalar payload (e.g. a field of a record payload) has
    // no slot to bind and must decline here rather than leak a deferred
    // structural-field reference into the emitted module.
    if crate::emission::case_payload_dispatch::direct_case_reads(
        &when_true_arguments,
        source,
        selected,
    ) || crate::emission::case_payload_dispatch::direct_case_reads(
        &when_true_erased_arguments,
        source,
        selected,
    ) {
        return unsupported("case dispatch cannot bind a non-scalar payload observation");
    }
    Ok(LoweredScalarBranchTerminator::CaseDispatch {
        source,
        selected,
        cases: cases.iter().map(|declared| declared.id).collect(),
        payloads,
        when_true_target,
        when_true_arguments,
        when_true_erased_arguments,
        when_true_erased_proof_arguments,
        when_false_target,
        when_false_arguments,
        when_false_erased_arguments,
        when_false_erased_proof_arguments,
    })
}
