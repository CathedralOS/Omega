use checked_trees::{
    CheckedScalarBinding, CheckedScalarBindingValue, CheckedScalarBranchDestination,
    CheckedScalarGraphPlans, CheckedScalarMachineGraph, CheckedScalarParameterStorage,
    CheckedScalarStateGraph, CheckedScalarStateTerminator, CheckedScalarSuccessor,
    CheckedTerminalMachineSelection, CheckedTerminalMachineSelections,
    CheckedTerminalSignatureEligibility,
};

pub(crate) fn build_checked_terminal_machine_selections(
    program: &TypedTrees,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> CheckedTerminalMachineSelections {
    CheckedTerminalMachineSelections {
        machines: program
            .machines()
            .iter()
            .map(|machine| CheckedTerminalMachineSelection {
                machine: machine.symbol,
                name: program.symbols.display_path(machine.symbol, "::"),
                // `satisfies` is checked requirement refinement, not a runtime
                // receiver or an alternate body. A closed checked machine keeps
                // its ordinary signature and its own published contract.
                signature: if machine.attached_data.is_some() {
                    CheckedTerminalSignatureEligibility::Attached
                } else if !machine.type_parameters.is_empty()
                    || !machine.owned_data.is_empty()
                    || (machine.termination_plan.implementation_witness.is_some()
                        && crate::checks::termination::proven_slice_length_ranks_with_call_frames(
                            program,
                            machine,
                            call_frames,
                        )
                        .is_none()
                        && crate::checks::termination::proven_nat_countdown_sccs_with_call_frames(
                            program,
                            machine,
                            call_frames,
                        )
                        .is_none_or(|components| components.is_empty()))
                    || machine.suspends
                    || machine.blocks
                    || !machine.supply_mode.is_checked_body()
                {
                    CheckedTerminalSignatureEligibility::Unsupported
                } else if !program
                    .service_reach_rows
                    .services(machine.service_reach_row)
                    .is_empty()
                    || !machine.invokes.is_empty()
                    || program.machine_states(machine).iter().any(|state| {
                        program.state_parameters(state).iter().any(|parameter| {
                            typed_trees::service::exact_bound_service_requirement(
                                program,
                                parameter.type_reference,
                            )
                            .is_some()
                        })
                    })
                {
                    CheckedTerminalSignatureEligibility::FreeUnitEffect
                } else {
                    CheckedTerminalSignatureEligibility::Eligible
                },
            })
            .collect(),
    }
}
use typed_trees::{
    TypedTrees,
    statement::{StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode},
};

mod constructions;
mod guarded_exits;
mod guards;
mod owned_parameters;
pub(super) mod primitive_locals;
mod unit_operations;
pub(crate) use unit_operations::finalize as finalize_scalar_unit_operations;
mod ranking;
mod successors;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn build_checked_scalar_graph_plans(
    program: &TypedTrees,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
    structural_values: &checked_trees::CheckedStructuralValuePlans,
    proof_terms: &checked_trees::CheckedProofTerms,
) -> CheckedScalarGraphPlans {
    build_checked_scalar_graph_plans_with_call_frames(
        program,
        expressions,
        computations,
        structural_values,
        proof_terms,
        None,
    )
}

pub(crate) fn build_checked_scalar_graph_plans_with_call_frames(
    program: &TypedTrees,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
    structural_values: &checked_trees::CheckedStructuralValuePlans,
    proof_terms: &checked_trees::CheckedProofTerms,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> CheckedScalarGraphPlans {
    let (guarded_exits, guarded_tails) = guarded_exits::build(program, expressions);
    let mut parameter_storage = arena::Arena::default();
    let mut structural_types = std::collections::BTreeMap::new();
    let mut machines: Vec<_> = program
        .machines()
        .iter()
        .filter_map(|machine| {
            build_machine_graph(
                program,
                machine,
                expressions,
                computations,
                structural_values,
                &mut parameter_storage,
                &mut structural_types,
            )
        })
        .collect();
    let mut structural_transfers = arena::Arena::default();
    let mut scalar_arguments = arena::Arena::default();
    let mut erased_proof_arguments = arena::Arena::default();
    // Resolve all edges before mutating spans or dropping machines: a named
    // cross-machine target reads its own machine's parameter partition, so
    // every graph in the completed list stays visible through resolution.
    let resolved_arguments = machines
        .iter()
        .map(|graph| {
            successors::resolve_arguments(program, expressions, proof_terms, &machines, graph)
        })
        .collect::<Vec<_>>();
    let mut resolved_arguments = resolved_arguments.into_iter();
    machines.retain_mut(|graph| {
        let rows = resolved_arguments.next().flatten();
        let Some(ranked_scc) = ranking::plan(program, graph, call_frames) else {
            return false;
        };
        graph.ranked_scc = ranked_scc;
        let Some(rows) = rows else {
            return false;
        };
        successors::commit_arguments(
            graph,
            rows,
            &mut structural_transfers,
            &mut scalar_arguments,
            &mut erased_proof_arguments,
        );
        true
    });
    CheckedScalarGraphPlans {
        machines,
        parameter_storage,
        structural_transfers,
        scalar_arguments,
        erased_proof_arguments,
        structural_types: structural_types.into_values().collect(),
        guarded_exits,
        guarded_tails,
    }
}

/// Finalize discovered bodies only after ownership checking supplies the ledger.
#[cfg(test)]
pub(crate) fn finalize_checked_scalar_graph_plans(
    program: &TypedTrees,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    ownership: &checked_trees::FlowOwnershipFacts,
    computations: &checked_trees::CheckedScalarComputationPlans,
    plans: &mut CheckedScalarGraphPlans,
    proof_terms: &checked_trees::CheckedProofTerms,
) {
    finalize_checked_scalar_graph_plans_with_call_frames(
        program,
        expressions,
        ownership,
        computations,
        plans,
        proof_terms,
        None,
    )
}

pub(crate) fn finalize_checked_scalar_graph_plans_with_call_frames(
    program: &TypedTrees,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    ownership: &checked_trees::FlowOwnershipFacts,
    computations: &checked_trees::CheckedScalarComputationPlans,
    plans: &mut CheckedScalarGraphPlans,
    proof_terms: &checked_trees::CheckedProofTerms,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) {
// Validation reads each successor's own machine graph for cross-machine
    // targets, so every graph is checked against the completed list before
    // any machine is dropped.
    let retained = plans
        .machines
        .iter()
        .map(|graph| {
            if ranking::plan(program, graph, call_frames) != Some(graph.ranked_scc.clone()) {
                return false;
            }
            if successors::validate(
                program,
                expressions,
                &plans.machines,
                graph,
                &plans.structural_transfers,
                &plans.scalar_arguments,
                &plans.erased_proof_arguments,
                proof_terms,
            )
            .is_none()
            {
                return false;
            }
            if crate::lookup::machine_by_symbol(program, graph.machine).is_none() {
                return false;
            }
            graph.states.iter().all(|retained| {
                // A fused graph may retain a state authored under a sibling
                // machine — custody predicates keep answering through the
                // state's own owner, not the dispatch machine.
                let Some((owner, state)) =
                    crate::semantic::calls::find_state_with_machine(program, retained.state)
                else {
                    return false;
                };
                let Some(transfers) = successors::owned_transfers(
                    program,
                    owner.symbol,
                    state,
                    retained,
                    &plans.structural_transfers,
                ) else {
                    return false;
                };
                owned_parameters::validate(
                    program,
                    ownership,
                    computations,
                    owner.symbol,
                    state,
                    &retained.structural_parameters,
                    &transfers,
                )
                .is_some()
            })
        })
        .collect::<Vec<_>>();
    let mut retained = retained.into_iter();
    plans.machines.retain(|_| retained.next().unwrap_or(false));
}

fn build_machine_graph(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
    structural_values: &checked_trees::CheckedStructuralValuePlans,
    parameter_storage: &mut arena::Arena<CheckedScalarParameterStorage>,
    structural_types: &mut std::collections::BTreeMap<
        String,
        checked_trees::CheckedUnitStructuralTypePlan,
    >,
) -> Option<CheckedScalarMachineGraph> {
    let source_states = program.machine_states(machine);
    if source_states.is_empty() {
        return None;
    }
    // A tail arm spelling another machine's entry (`-> self.pong`) stays inside
    // the one dispatch loop, so every state its edges can reach joins this
    // graph: the worklist pulls the owning machine's whole state list whenever
    // a successor names a foreign entry, transitively. States keep their own
    // owning machine's parameters and contracts; only the edge list is fused.
    let mut states = Vec::new();
    let mut pending: Vec<(&typed_trees::machine::Machine, &typed_trees::state::State)> =
        source_states.iter().map(|state| (machine, state)).collect();
    let mut cursor = 0;
    while let Some(&(owner, state)) = pending.get(cursor) {
        cursor += 1;
        if states
            .iter()
            .any(|(candidate, _, _): &(CheckedScalarStateGraph, _, _)| {
                candidate.state == state.symbol
            })
        {
            continue;
        }
        let owner_states = program.machine_states(owner);
        let built = checked_state_graph(
            program,
            owner,
            state,
            owner_states.len(),
            expressions,
            computations,
            structural_values,
        )?;
        for successor in successors::iter(&built.0.terminator) {
            if pending
                .iter()
                .any(|(_, candidate)| candidate.symbol == successor.target)
            {
                continue;
            }
            let Some((target_owner, _)) =
                crate::semantic::calls::find_machine_by_entry_state(program, successor.target)
            else {
                continue;
            };
            for owner_state in program.machine_states(target_owner) {
                if !pending
                    .iter()
                    .any(|(_, candidate)| candidate.symbol == owner_state.symbol)
                {
                    pending.push((target_owner, owner_state));
                }
            }
        }
        states.push(built);
    }
    // The ambient receiver owns a graph operand only when the entry roster
    // retained it; a state that still reads `self` otherwise keeps its
    // ordinary body rather than publishing stranded receiver references.
    if !states.iter().any(|(state, ..)| {
        state
            .structural_parameters
            .iter()
            .any(|parameter| parameter.is_self)
    }) {
        for &(_, state) in &pending {
            let parameters = program.state_parameters(state);
            if let Some(position) = parameters.iter().position(|parameter| parameter.is_self) {
                let Ok(position) = u32::try_from(position) else {
                    return None;
                };
                if state_reads_ambient_position(expressions, computations, state.symbol, position) {
                    return None;
                }
            }
        }
    }
    Some(CheckedScalarMachineGraph {
        machine: machine.symbol,
        ranked_scc: None,
        // Commit storage rows only after the complete machine shape succeeds.
        states: states
            .into_iter()
            .map(|(mut state, storage, shapes)| {
                state.parameter_storage = parameter_storage.insert_many(storage);
                for shape in shapes {
                    structural_types.insert(shape.identity.clone(), shape);
                }
                state
            })
            .collect(),
    })
}

fn checked_state_graph(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    owner_state_count: usize,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
    structural_values: &checked_trees::CheckedStructuralValuePlans,
) -> Option<(
    CheckedScalarStateGraph,
    Vec<CheckedScalarParameterStorage>,
    Vec<checked_trees::CheckedUnitStructuralTypePlan>,
)> {
    {
        {
            if !validation::scalar_state_contracts_are_qualifications(program, state) {
                return None;
            }
            let parameters = program.state_parameters(state);
            // A borrowed `self` stays ambient on the attachment carrier: the
            // scalar graph keeps no structural or scalar slot for it, the same
            // convention the completed Unit signature uses when it does not
            // retain the receiver. An owned `self` still has no graph
            // representation, so it refuses the plan.
            if parameters.iter().any(|parameter| {
                super::terminal_unit::strips_erased_parameter(parameter).is_none()
                    || (parameter.is_self
                        && !crate::borrow::view_link::is_reference_type(
                            program,
                            parameter.type_reference,
                        ))
                    || parameter.is_const
                    || (parameter.is_mutable
                        && !matches!(
                            program
                                .type_reference_table
                                .type_reference(parameter.type_reference),
                            typed_trees::types::TypeReferenceNode::Reference { .. }
                        )
                        && crate::values::mutable_scalar_parameter_type(program, parameter)
                            .is_none())
            }) {
                return None;
            }
            // An erased parameter (a plain immutable binding after the
            // rejections above) owns no scalar entry and does not make the
            // signature mixed, whatever its type: a proof-only type such as
            // `Nat` is exactly what erasure exists for. Neither does the
            // ambient borrowed receiver, which is not a value argument.
            let mixed = parameters.iter().any(|parameter| {
                !parameter.relevance.is_erased()
                    && !parameter.is_self
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            });
            let source_states = program.machine_states(machine);
            let (structural_parameters, scalar_parameters, mut shapes) = if mixed
                && machine.attached_data.is_some()
                && !parameters
                    .iter()
                    .any(|parameter| parameter.is_self && parameter.is_mutable)
            {
                // An attached machine's graph carries the whole mixed
                // signature per state: the ambient receiver lands on the
                // entry roster while each state's own structural formals
                // forward across the edges that reach it. The carrier
                // contract stays the ordinary scalar-graph admission, so a
                // structural parameter the graph cannot carry keeps the
                // whole machine off the graph.
                if !crate::execution::terminal_unit::structural_scalar_graph_parameter_admission(
                    program, state,
                ) {
                    return None;
                }
                crate::execution::terminal_unit::calls::mixed_ambient_scalar_graph_signature(
                    program,
                    machine,
                    state,
                    source_states[0].symbol,
                )?
            } else if mixed {
                // Whole structural forwarding for a free machine is bounded
                // to the same authored state; additional state signatures
                // remain a separate slice.
                if owner_state_count != 1 || machine.attached_data.is_some() {
                    return None;
                }
                super::terminal_unit::structural_scalar_graph_signature(program, state)?
            } else if state.symbol == source_states[0].symbol
                && parameters
                    .iter()
                    .any(|parameter| parameter.is_self && !parameter.is_mutable)
            {
                // The entry roster doubles as the machine's structural
                // namespace, so a borrowed receiver lands there once: every
                // state's reads resolve against that machine-level place
                // while no edge ever forwards it.
                crate::execution::terminal_unit::calls::ambient_self_scalar_graph_signature(
                    program, machine, state,
                )?
            } else {
                (
                    Vec::new(),
                    parameters
                        .iter()
                        .enumerate()
                        .filter(|(_, parameter)| {
                            !parameter.relevance.is_erased() && !parameter.is_self
                        })
                        .map(|(position, parameter)| {
                            Some(checked_trees::CheckedStructuralScalarParameterPlan {
                                source_position: u32::try_from(position).ok()?,
                                primitive_type: program
                                    .primitive_type_reference(parameter.type_reference)?,
                            })
                        })
                        .collect::<Option<Vec<_>>>()?,
                    Vec::new(),
                )
            };
            // Proof-only erased formals leave the scalar roster and index the
            // contract term lane instead; scalar erased ordinals stay dense
            // over scalar carriers only.
            let erased_scalar_parameters =
                crate::execution::terminal_unit::types::erased_scalar_parameter_plans(
                    program, state,
                )?;
            let erased_proof_parameters =
                crate::execution::terminal_unit::types::erased_proof_parameter_plans(
                    program, state,
                )?;
            let parameter_types = scalar_parameters
                .iter()
                .map(|parameter| parameter.primitive_type)
                .collect();
            constructions::retain_shapes(program, computations, state.symbol, &mut shapes)?;
            constructions::retain_record_locals(
                program,
                structural_values,
                machine,
                state,
                &mut shapes,
            )?;
            let storage = parameters
                .iter()
                .enumerate()
                // Mutable borrows already have structural parameter places;
                // only authored mutable scalar formals need local storage.
                .filter(|(_, parameter)| {
                    parameter.is_mutable
                        && !parameter.relevance.is_erased()
                        && program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                })
                .map(|(ordinal, parameter)| {
                    Some(CheckedScalarParameterStorage {
                        parameter_ordinal: u32::try_from(ordinal).ok()?,
                        symbol: parameter.symbol,
                        primitive_type: crate::values::mutable_scalar_parameter_type(
                            program, parameter,
                        )?,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            let result_type = program.primitive_type_reference(state.return_type)?;
            let statements = program.statement_table.statements(state.statement_nodes);
            let bindings = checked_statement_bindings(program, state, computations, true)?;
            // Call-free primitive-reference leaves retain their existing
            // structural scalar-return owner, including its admission fences.
            if mixed
                && structural_parameters.iter().all(|parameter| {
                    parameter.access != checked_trees::CheckedStructuralAccess::Owned
                })
                && matches!(statements, [StatementNode::Expression(_)])
                && !computations
                    .roots
                    .iter()
                    .any(|(_, root)| root.state == state.symbol)
            {
                return None;
            }
            let primitive_locals = primitive_locals::collect(
                program,
                machine,
                state,
                expressions,
                computations,
                &bindings,
            )?;
            if !primitive_locals.is_empty()
                && (owner_state_count != 1 || machine.attached_data.is_some())
            {
                return None;
            }
            for local in &primitive_locals {
                let shape = checked_trees::CheckedUnitStructuralTypePlan {
                    identity: local.type_identity.clone(),
                    shape: checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(
                        local.primitive_type,
                    ),
                };
                if let Some(existing) = shapes.iter().find(|entry| entry.identity == shape.identity)
                {
                    if existing != &shape {
                        return None;
                    }
                } else {
                    shapes.push(shape);
                }
            }
            let binding_count = statements
                .iter()
                .take_while(|statement| {
                    matches!(
                        statement,
                        StatementNode::LocalData(_)
                            | StatementNode::Assignment(_)
                            | StatementNode::Call(_)
                    )
                })
                .count();
            let terminator =
                checked_terminator(program, machine, state, expressions, binding_count)?;
            // Call syntax at a return is not a scalar-graph value producer.
            // Calls owned by the operation sequence (including boundaries)
            // must not acquire a competing graph with an unbound return.
            if let CheckedScalarStateTerminator::Return { statement_ordinal } = &terminator
                && matches!(statements.get(*statement_ordinal as usize),
                    Some(StatementNode::Expression(expression)) if matches!(
                        program.expression_table.expression(*expression),
                        typed_trees::expression::ExpressionNode::Call(_)))
                && expressions
                    .expression_at(
                        state.symbol,
                        *statement_ordinal,
                        checked_trees::CheckedScalarExpressionRole::Return,
                    )
                    .is_none()
                && computations
                    .root_at(
                        state.symbol,
                        *statement_ordinal,
                        checked_trees::CheckedScalarExpressionRole::Return,
                    )
                    .is_none()
            {
                return None;
            }
            Some((
                CheckedScalarStateGraph {
                    state: state.symbol,
                    structural_parameters,
                    scalar_parameters,
                    erased_scalar_parameters,
                    erased_proof_parameters,
                    parameter_types,
                    parameter_storage: arena::HandleSpan::empty(),
                    primitive_locals,
                    bindings,
                    unit_operations: Vec::new(),
                    result_type,
                    terminator,
                },
                storage,
                shapes,
            ))
        }
    }
}

/// Retain the contiguous authored declaration/assignment prefix in the shared
/// scalar binding representation. Consumers choose the supported value forms.
pub(super) fn checked_binding_prefix(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    computations: &checked_trees::CheckedScalarComputationPlans,
) -> Option<Vec<CheckedScalarBinding>> {
    checked_statement_bindings(program, state, computations, false)
}

fn checked_statement_bindings(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    computations: &checked_trees::CheckedScalarComputationPlans,
    unit_calls: bool,
) -> Option<Vec<CheckedScalarBinding>> {
    let parameters = program.state_parameters(state);
    let statements = program.statement_table.statements(state.statement_nodes);
    let binding_count = statements
        .iter()
        .take_while(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(_) | StatementNode::Assignment(_)
            ) || (unit_calls && matches!(statement, StatementNode::Call(_)))
        })
        .count();
    let bindings =
        statements[..binding_count]
            .iter()
            .enumerate()
            .filter(|(_, statement)| !matches!(statement, StatementNode::Call(_))
                && !(unit_calls && matches!(statement, StatementNode::Assignment(assignment)
                    if matches!(program.expression_table.expression(assignment.target),
                        typed_trees::expression::ExpressionNode::Member(_))))
                && !(unit_calls && matches!(statement, StatementNode::LocalData(local)
                    if program.primitive_type_reference(local.type_reference).is_none())))
            .map(|(statement_index, statement)| {
                use checked_trees::CheckedScalarBindingDestination;
                match statement {
                    StatementNode::LocalData(local) => {
                        if !program
                            .expression_table
                            .expression_is_valid(local.initial_value)
                        {
                            return None;
                        }
                        let statement_ordinal = u32::try_from(statement_index).ok()?;
                        let role = if local.is_mutable {
                            checked_trees::CheckedScalarExpressionRole::StorageInitializer
                        } else {
                            let preceding_immutable_count = statements[..statement_index].iter().filter(|statement| {
                                matches!(statement, StatementNode::LocalData(local) if !local.is_mutable
                                    && program.primitive_type_reference(local.type_reference).is_some())
                            }).count();
                            checked_trees::CheckedScalarExpressionRole::LocalInitializer {
                                binding_ordinal: u32::try_from(preceding_immutable_count).ok()?,
                            }
                        };
                        let value = if computations
                            .root_at(state.symbol, statement_ordinal, role)
                            .is_some()
                        {
                            CheckedScalarBindingValue::Computation
                        } else {
                            checked_binding_value(program, local.initial_value)?
                        };
                        if local.is_mutable
                            && matches!(value, CheckedScalarBindingValue::DirectCall { .. })
                        {
                            return None;
                        }
                        Some(CheckedScalarBinding {
                            statement_ordinal,
                            destination: if local.is_mutable {
                                CheckedScalarBindingDestination::StorageInitialize {
                                    symbol: local.symbol,
                                }
                            } else {
                                CheckedScalarBindingDestination::Immutable
                            },
                            primitive_type: program
                                .primitive_type_reference(local.type_reference)?,
                            value,
                        })
                    }
                    StatementNode::Assignment(assignment) => {
                        let typed_trees::expression::ExpressionNode::Name(name) =
                            program.expression_table.expression(assignment.target)
                        else {
                            return None;
                        };
                        if !name.symbol.is_valid() || name.head_symbol != name.symbol {
                            return None;
                        }
                        let destination = statements[..statement_index]
                            .iter()
                            .find_map(|statement| match statement {
                                StatementNode::LocalData(local)
                                    if local.symbol == name.symbol && local.is_mutable =>
                                {
                                    Some((
                                        local.symbol,
                                        program.primitive_type_reference(local.type_reference)?,
                                    ))
                                }
                                _ => None,
                            })
                            .or_else(|| {
                                parameters.iter().find_map(|parameter| {
                                    (parameter.symbol == name.symbol)
                                        .then(|| {
                                            Some((
                                                parameter.symbol,
                                                crate::values::mutable_scalar_parameter_type(
                                                    program, parameter,
                                                )?,
                                            ))
                                        })
                                        .flatten()
                                })
                            })?;
                        let statement_ordinal = u32::try_from(statement_index).ok()?;
                        let role = checked_trees::CheckedScalarExpressionRole::AssignmentValue;
                        let value = if computations
                            .root_at(state.symbol, statement_ordinal, role)
                            .is_some()
                        {
                            CheckedScalarBindingValue::Computation
                        } else {
                            CheckedScalarBindingValue::Expression
                        };
                        Some(CheckedScalarBinding {
                            statement_ordinal,
                            destination: CheckedScalarBindingDestination::StorageAssign {
                                symbol: destination.0,
                            },
                            primitive_type: destination.1,
                            value,
                        })
                    }
                    _ => None,
                }
            })
            .collect::<Option<Vec<_>>>()?;
    Some(bindings)
}

fn checked_binding_value(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedScalarBindingValue> {
    let typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(expression)
    else {
        return Some(CheckedScalarBindingValue::Expression);
    };
    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
        return None;
    }
    let (target_machine, target_state) =
        crate::semantic::calls::find_machine_by_entry_state(program, call.target_symbol)?;
    let parameters = program.state_parameters(target_state);
    let authored_arguments = program
        .expression_table
        .expression_handles(call.arguments)
        .len();
    if authored_arguments != parameters.len() {
        return None;
    }
    // The retained argument count. An erased position has no caller argument
    // ordinal (`values::scalar::call_lowering` numbers `CallArgument` roles
    // over the retained positions only), matching the callee's stripped
    // scalar signature.
    let mut retained_arguments = 0usize;
    for parameter in parameters {
        if !super::terminal_unit::strips_erased_parameter(parameter)? {
            retained_arguments = retained_arguments.checked_add(1)?;
        }
    }
    Some(CheckedScalarBindingValue::DirectCall {
        target_machine: target_machine.symbol,
        target_state: call.target_symbol,
        // A supported call is the root of its local initializer. Nested calls
        // cannot acquire scalar argument plans and therefore fail closed.
        call_ordinal: 0,
        argument_count: u32::try_from(retained_arguments).ok()?,
    })
}

fn checked_successor(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    statement_ordinal: u32,
    transition: &typed_trees::statement::TableTransition,
    is_continuation: bool,
) -> Option<CheckedScalarSuccessor> {
    if transition.exit != TransitionExit::Ordinary {
        return None;
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program
        .statement_table
        .transition_target(if is_continuation {
            transition.continuation
        } else {
            transition.target
        })
    else {
        return None;
    };
    // An authored machine-name backedge denotes its entry child. Reuse the
    // same identity normalization as ranking and structural state forwarding.
    let target = if let Some(target_index) =
        crate::checks::termination::named_transition_target_state_index(
            program,
            machine,
            path.symbol,
        ) {
        program.machine_states(machine).get(target_index)?.symbol
    } else {
        // A target spelling another machine's entry names that machine's
        // first state outright (`-> self.pong` carries `pong`'s entry-state
        // symbol): the same spelling the scalar call-lowering gates resolve
        // through `find_machine_by_entry_state`.
        crate::semantic::calls::find_machine_by_entry_state(program, path.symbol)?.1
            .symbol
    };
    Some(CheckedScalarSuccessor {
        statement_ordinal,
        is_continuation,
        target,
        argument_count: u32::try_from(program.statement_table.expression_handles(*arguments).len())
            .ok()?,
        structural_transfers: arena::HandleSpan::empty(),
        scalar_arguments: arena::HandleSpan::empty(),
        erased_arguments: arena::HandleSpan::empty(),
        erased_proof_arguments: arena::HandleSpan::empty(),
    })
}

/// Retain authored exit coordinates independently of the preceding operation values.
pub(super) fn checked_terminator(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    binding_count: usize,
) -> Option<CheckedScalarStateTerminator> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let terminator_ordinal = u32::try_from(binding_count).ok()?;
    Some(match &statements[binding_count..] {
        [StatementNode::Expression(_)] => CheckedScalarStateTerminator::Return {
            statement_ordinal: terminator_ordinal,
        },
        [StatementNode::Transition(transition)]
            if transition.exit == TransitionExit::Ordinary
                && transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid()
                && matches!(
                    program.statement_table.transition_target(transition.target),
                    TransitionTargetNode::Value(_)
                ) =>
        {
            CheckedScalarStateTerminator::Return {
                statement_ordinal: terminator_ordinal,
            }
        }
        [StatementNode::Transition(transition)]
            if matches!(transition.exit, TransitionExit::Crash(_))
                && transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid()
                && program
                    .statement_table
                    .transition_target_is_valid(transition.target)
                && matches!(
                    program.statement_table.transition_target(transition.target),
                    TransitionTargetNode::Terminal
                ) =>
        {
            CheckedScalarStateTerminator::Crash {
                statement_ordinal: terminator_ordinal,
            }
        }
        [
            StatementNode::Transition(when_true),
            StatementNode::Transition(when_false),
        ] if matches!(when_true.guard, TransitionGuardNode::When(_))
            && (when_false.guard == TransitionGuardNode::Always
                || guards::complementary(expressions, state.symbol, terminator_ordinal))
            && !when_true.continuation.is_valid()
            && !when_false.continuation.is_valid() =>
        {
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal: terminator_ordinal,
                when_true: checked_branch_destination(
                    program,
                    machine,
                    terminator_ordinal,
                    when_true,
                    false,
                )?,
                when_false: checked_branch_destination(
                    program,
                    machine,
                    terminator_ordinal.checked_add(1)?,
                    when_false,
                    false,
                )?,
            }
        }
        [StatementNode::Transition(transition)]
            if matches!(transition.guard, TransitionGuardNode::When(_))
                && transition.continuation.is_valid() =>
        {
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal: terminator_ordinal,
                when_true: checked_branch_destination(
                    program,
                    machine,
                    terminator_ordinal,
                    transition,
                    false,
                )?,
                when_false: checked_branch_destination(
                    program,
                    machine,
                    terminator_ordinal,
                    transition,
                    true,
                )?,
            }
        }
        [
            StatementNode::Transition(transition),
            StatementNode::Expression(_),
        ] if matches!(transition.guard, TransitionGuardNode::When(_))
            && !transition.continuation.is_valid() =>
        {
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal: terminator_ordinal,
                when_true: checked_branch_destination(
                    program,
                    machine,
                    terminator_ordinal,
                    transition,
                    false,
                )?,
                when_false: CheckedScalarBranchDestination::Return {
                    statement_ordinal: terminator_ordinal.checked_add(1)?,
                    is_continuation: false,
                },
            }
        }
        [StatementNode::Transition(transition)]
            if transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid() =>
        {
            CheckedScalarStateTerminator::Jump(checked_successor(
                program,
                machine,
                terminator_ordinal,
                transition,
                false,
            )?)
        }
        _ => return None,
    })
}

fn checked_branch_destination(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    statement_ordinal: u32,
    transition: &typed_trees::statement::TableTransition,
    is_continuation: bool,
) -> Option<CheckedScalarBranchDestination> {
    if matches!(transition.exit, TransitionExit::Crash(_)) {
        return (!is_continuation
            && !transition.continuation.is_valid()
            && program
                .statement_table
                .transition_target_is_valid(transition.target)
            && matches!(
                program.statement_table.transition_target(transition.target),
                TransitionTargetNode::Terminal
            ))
        .then_some(CheckedScalarBranchDestination::Crash { statement_ordinal });
    }
    let target = if is_continuation {
        transition.continuation
    } else {
        transition.target
    };
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Value(expression)
            if program.expression_table.expression_is_valid(*expression) =>
        {
            Some(CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            })
        }
        TransitionTargetNode::Named { .. } => checked_successor(
            program,
            machine,
            statement_ordinal,
            transition,
            is_continuation,
        )
        .map(CheckedScalarBranchDestination::Jump),
        _ => None,
    }
}

/// Whether any scalar plan rooted in `state` observes through the authored
/// parameter `position` — the ambient borrowed receiver's coordinate. An
/// unrecognized or malformed node counts as a read so the gate stays closed.
fn state_reads_ambient_position(
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
    state: symbols::SymbolHandle,
    position: u32,
) -> bool {
    expressions.expressions.iter().any(|expression| {
        expression.state == state
            && scalar_expression_reads_position(&expression.expression, position)
    }) || computations.roots.iter().any(|(_, root)| {
        root.state == state && computation_reads_position(computations, root.root, position)
    })
}

fn computation_reads_position(
    computations: &checked_trees::CheckedScalarComputationPlans,
    root: checked_trees::CheckedScalarComputationHandle,
    position: u32,
) -> bool {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        if !computations.nodes.is_valid(handle) {
            return true;
        }
        match &computations.nodes.get(handle).kind {
            checked_trees::CheckedScalarComputationKind::StructuralField { subject, .. } => {
                if structural_place_reads_position(subject, position) {
                    return true;
                }
            }
            checked_trees::CheckedScalarComputationKind::CaseMembership { subject, .. } => {
                if structural_argument_reads_position(computations, subject, position, &mut pending)
                {
                    return true;
                }
            }
            checked_trees::CheckedScalarComputationKind::SelectedComparison {
                left, right, ..
            } => pending.extend([*left, *right]),
            checked_trees::CheckedScalarComputationKind::Qualification { operand, .. }
            | checked_trees::CheckedScalarComputationKind::BooleanToInteger { operand, .. } => {
                pending.push(*operand)
            }
            checked_trees::CheckedScalarComputationKind::Value(expression) => {
                if scalar_expression_reads_position(expression, position) {
                    return true;
                }
            }
            checked_trees::CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                pending.push(*subject);
                let Some(arms) = computations.dispatch_arms.span(*arms) else {
                    return true;
                };
                for arm in arms {
                    if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern
                    {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            checked_trees::CheckedScalarComputationKind::Call {
                arguments,
                structural_arguments,
                ..
            } => {
                let Some(operands) = computations.operands.span(*arguments) else {
                    return true;
                };
                pending.extend_from_slice(operands);
                let Some(arguments) = computations
                    .structural_arguments
                    .span(*structural_arguments)
                else {
                    return true;
                };
                for argument in arguments {
                    if structural_argument_reads_position(
                        computations,
                        argument,
                        position,
                        &mut pending,
                    ) {
                        return true;
                    }
                }
            }
            checked_trees::CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => pending.extend([*condition, *when_true, *when_false]),
            checked_trees::CheckedScalarComputationKind::Apply {
                expression,
                operands,
                ..
            } => {
                if scalar_expression_reads_position(expression, position) {
                    return true;
                }
                let Some(operands) = computations.operands.span(*operands) else {
                    return true;
                };
                pending.extend_from_slice(operands);
            }
        }
    }
    false
}

fn structural_argument_reads_position(
    computations: &checked_trees::CheckedScalarComputationPlans,
    argument: &checked_trees::CheckedScalarComputationStructuralArgument,
    position: u32,
    pending: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
) -> bool {
    match argument {
        checked_trees::CheckedScalarComputationStructuralArgument::Place(place) => {
            structural_place_reads_position(place, position)
        }
        checked_trees::CheckedScalarComputationStructuralArgument::Case(construction) => {
            let Some(fields) = computations.case_fields.span(construction.fields) else {
                return true;
            };
            pending.extend(fields.iter().map(|field| field.value));
            false
        }
        checked_trees::CheckedScalarComputationStructuralArgument::Array { elements, .. } => {
            let Some(elements) = computations.operands.span(*elements) else {
                return true;
            };
            pending.extend_from_slice(elements);
            false
        }
    }
}

fn structural_place_reads_position(
    plan: &checked_trees::CheckedUnitStructuralArgumentPlan,
    position: u32,
) -> bool {
    if plan.source_parameter_index() == Some(position) {
        return true;
    }
    if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
        parameter_index,
        start,
        end,
        ..
    }
    | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
        parameter_index,
        start,
        end,
        ..
    } = &plan.source
    {
        return *parameter_index == position
            || start
                .as_ref()
                .is_some_and(|start| scalar_expression_reads_position(start, position))
            || end
                .as_ref()
                .is_some_and(|end| scalar_expression_reads_position(end, position));
    }
    false
}

fn scalar_expression_reads_position(
    expression: &checked_trees::CheckedScalarExpression,
    position: u32,
) -> bool {
    use checked_trees::CheckedScalarExpression as Scalar;
    match expression {
        Scalar::StructuralParameterByteLength {
            parameter_position, ..
        }
        | Scalar::StructuralParameterField {
            parameter_position, ..
        } => *parameter_position == position,
        Scalar::StructuralParameterIndexedRead {
            parameter_position,
            index,
            ..
        } => *parameter_position == position || scalar_expression_reads_position(index, position),
        Scalar::IntegerBinary { left, right, .. } => {
            scalar_expression_reads_position(left, position)
                || scalar_expression_reads_position(right, position)
        }
        Scalar::IntegerBitwiseNot { operand, .. }
        | Scalar::IntegerWiden { operand, .. }
        | Scalar::IntegerExactCast { operand, .. }
        | Scalar::IntegerWrappingCast { operand, .. }
        | Scalar::IntegerSaturatingCast { operand, .. }
        | Scalar::IntegerTrappingCast { operand, .. } => {
            scalar_expression_reads_position(operand, position)
        }
        Scalar::Boolean(expression) => boolean_expression_reads_position(expression, position),
        Scalar::StorageRead { .. }
        | Scalar::Parameter { .. }
        | Scalar::ErasedParameter { .. }
        | Scalar::Local { .. }
        | Scalar::IntegerLiteral { .. }
        | Scalar::IeeeFloatLiteral { .. } => false,
    }
}

fn boolean_expression_reads_position(
    expression: &checked_trees::CheckedBooleanExpression,
    position: u32,
) -> bool {
    use checked_trees::CheckedBooleanExpression as Boolean;
    match expression {
        Boolean::StructuralParameterField {
            parameter_position, ..
        } => *parameter_position == position,
        Boolean::IeeeFloatComparison { left, right, .. }
        | Boolean::ByteSequenceEqual { left, right }
        | Boolean::PayloadlessSumEqual { left, right, .. } => {
            left.parameter_position == position || right.parameter_position == position
        }
        Boolean::StructuralCaseMembership { subject, .. } => subject.parameter_position == position,
        Boolean::Not(operand) => boolean_expression_reads_position(operand, position),
        Boolean::Equal { left, right }
        | Boolean::And { left, right }
        | Boolean::Or { left, right } => {
            boolean_expression_reads_position(left, position)
                || boolean_expression_reads_position(right, position)
        }
        Boolean::IntegerComparison { left, right, .. }
        | Boolean::ScalarIeeeFloatComparison { left, right, .. } => {
            scalar_expression_reads_position(left, position)
                || scalar_expression_reads_position(right, position)
        }
        Boolean::StorageRead { .. }
        | Boolean::Constant(_)
        | Boolean::Parameter { .. }
        | Boolean::ErasedParameter { .. }
        | Boolean::Local { .. } => false,
    }
}
