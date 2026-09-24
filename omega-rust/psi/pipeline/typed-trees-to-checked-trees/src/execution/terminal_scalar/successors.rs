//! Exact authored mixed arguments on scalar state edges.

use arena::Arena;
use checked_trees::{
    CheckedProofTerm, CheckedScalarBranchDestination, CheckedScalarExpressionPlans,
    CheckedScalarMachineGraph, CheckedScalarStateGraph, CheckedScalarStateTerminator,
    CheckedScalarSuccessor, CheckedStructuralAccess, CheckedStructuralControlTransferPlan,
    CheckedStructuralControlTransferSourcePlan, CheckedStructuralScalarArgumentPlan,
    CheckedStructuralScalarArgumentSourcePlan,
};
use language_semantics::{Multiplicity, PermissionEventSource};
use typed_trees::{
    TypedTrees,
    expression::ExpressionNode,
    state::State,
    statement::{StatementNode, TransitionExit, TransitionTargetNode},
};

pub(super) struct SuccessorArguments {
    structural: Vec<CheckedStructuralControlTransferPlan>,
    scalar: Vec<CheckedStructuralScalarArgumentPlan>,
    erased: Vec<CheckedStructuralScalarArgumentPlan>,
    proof: Vec<CheckedProofTerm>,
}

pub(super) fn iter(
    terminator: &CheckedScalarStateTerminator,
) -> impl Iterator<Item = &CheckedScalarSuccessor> {
    fn branch(destination: &CheckedScalarBranchDestination) -> Option<&CheckedScalarSuccessor> {
        match destination {
            CheckedScalarBranchDestination::Jump(successor) => Some(successor),
            _ => None,
        }
    }
    match terminator {
        CheckedScalarStateTerminator::Jump(successor) => [Some(successor), None],
        CheckedScalarStateTerminator::Conditional {
            when_true,
            when_false,
            ..
        } => [branch(when_true), branch(when_false)],
        _ => [None, None],
    }
    .into_iter()
    .flatten()
}

fn iter_mut(
    terminator: &mut CheckedScalarStateTerminator,
) -> impl Iterator<Item = &mut CheckedScalarSuccessor> {
    fn branch(
        destination: &mut CheckedScalarBranchDestination,
    ) -> Option<&mut CheckedScalarSuccessor> {
        match destination {
            CheckedScalarBranchDestination::Jump(successor) => Some(successor),
            _ => None,
        }
    }
    match terminator {
        CheckedScalarStateTerminator::Jump(successor) => [Some(successor), None],
        CheckedScalarStateTerminator::Conditional {
            when_true,
            when_false,
            ..
        } => [branch(when_true), branch(when_false)],
        _ => [None, None],
    }
    .into_iter()
    .flatten()
}

/// Resolve every edge's argument partition before any span mutates: a named
/// cross-machine target reads its own machine's parameter partition, so the
/// completed graph list stays immutable through resolution.
pub(super) fn resolve_arguments(
    program: &TypedTrees,
    expressions: &CheckedScalarExpressionPlans,
    proof_terms: &checked_trees::CheckedProofTerms,
    graphs: &[CheckedScalarMachineGraph],
    graph: &CheckedScalarMachineGraph,
) -> Option<Vec<SuccessorArguments>> {
    graph
        .states
        .iter()
        .flat_map(|source| {
            iter(&source.terminator).map(move |successor| {
                arguments(
                    program,
                    expressions,
                    proof_terms,
                    graphs,
                    graph,
                    source,
                    successor,
                )
            })
        })
        .collect()
}

/// Write resolved edge arguments into the durable arenas.
pub(super) fn commit_arguments(
    graph: &mut CheckedScalarMachineGraph,
    rows: Vec<SuccessorArguments>,
    structural: &mut Arena<CheckedStructuralControlTransferPlan>,
    scalar: &mut Arena<CheckedStructuralScalarArgumentPlan>,
    proof: &mut Arena<CheckedProofTerm>,
) {
    for (successor, rows) in graph
        .states
        .iter_mut()
        .flat_map(|source| iter_mut(&mut source.terminator))
        .zip(rows)
    {
        successor.structural_transfers = structural.insert_many(rows.structural);
        successor.scalar_arguments = scalar.insert_many(rows.scalar);
        successor.erased_arguments = scalar.insert_many(rows.erased);
        successor.erased_proof_arguments = proof.insert_many(rows.proof);
    }
}

pub(super) fn validate(
    program: &TypedTrees,
    expressions: &CheckedScalarExpressionPlans,
    graphs: &[CheckedScalarMachineGraph],
    graph: &CheckedScalarMachineGraph,
    structural: &Arena<CheckedStructuralControlTransferPlan>,
    scalar: &Arena<CheckedStructuralScalarArgumentPlan>,
    proof: &Arena<CheckedProofTerm>,
    proof_terms: &checked_trees::CheckedProofTerms,
) -> Option<()> {
    for source in &graph.states {
        for successor in iter(&source.terminator) {
            let expected = arguments(
                program,
                expressions,
                proof_terms,
                graphs,
                graph,
                source,
                successor,
            )?;
            if structural.span(successor.structural_transfers)? != expected.structural
                || scalar.span(successor.scalar_arguments)? != expected.scalar
                || scalar.span(successor.erased_arguments)? != expected.erased
                || proof.span(successor.erased_proof_arguments)? != expected.proof
            {
                return None;
            }
        }
    }
    Some(())
}

fn arguments<'a>(
    program: &TypedTrees,
    expressions: &CheckedScalarExpressionPlans,
    proof_terms: &checked_trees::CheckedProofTerms,
    graphs: &'a [CheckedScalarMachineGraph],
    graph: &'a CheckedScalarMachineGraph,
    source: &'a CheckedScalarStateGraph,
    successor: &'a CheckedScalarSuccessor,
) -> Option<SuccessorArguments> {
    let machine = crate::lookup::machine_by_symbol(program, graph.machine)?;
    let states = program.machine_states(machine);
    // A fused graph retains a sibling machine's states beside its own, so a
    // retained state may be authored under another owner: resolve authored
    // states through the whole-program owner lookup before reaching for the
    // foreign call-signature shape.
    let (source_machine, source_state) =
        match states.iter().find(|state| state.symbol == source.state) {
            Some(source_state) => (machine, source_state),
            None => crate::semantic::calls::find_state_with_machine(program, source.state)?,
        };
    let (target_machine, target_state, target) = if let Some(target) = graph
        .states
        .iter()
        .find(|state| state.state == successor.target)
    {
        let (target_machine, target_state) =
            match states.iter().find(|state| state.symbol == target.state) {
                Some(target_state) => (machine, target_state),
                None => crate::semantic::calls::find_state_with_machine(program, target.state)?,
            };
        (target_machine, target_state, target)
    } else {
        // A successor spelling another machine's entry names that machine's
        // first state; read its parameter partition through that machine's
        // own graph.
        let (target_machine, target_state) =
            crate::semantic::calls::find_machine_by_entry_state(program, successor.target)?;
        let target = graphs
            .iter()
            .find(|candidate| candidate.machine == target_machine.symbol)?
            .states
            .iter()
            .find(|state| state.state == successor.target)?;
        (target_machine, target_state, target)
    };
    let target_states = program.machine_states(target_machine);
    let source_parameters = program.state_parameters(source_state);
    let target_parameters = program.state_parameters(target_state);
    let forwarded = |state: &CheckedScalarStateGraph| {
        state
            .structural_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count()
    };
    if forwarded(source) != 0 || forwarded(target) != 0 {
        // An attached machine without a mutable receiver carries the mixed
        // signature per state: the ambient `&self` stays on the entry roster
        // while each state's own structural formals forward on its incoming
        // edges. Other rosters keep the single-state bound.
        // Rosters are authored per state owner: a fused or cross-machine edge
        // resolves each side's signature under its own machine.
        let ambient_roster = |owner_machine: &typed_trees::machine::Machine,
                              owner_states: &[typed_trees::state::State],
                              state: &CheckedScalarStateGraph| {
            let typed_state = owner_states
                .iter()
                .find(|entry| entry.symbol == state.state)?;
            if owner_machine.attached_data.is_none()
                || program
                    .state_parameters(typed_state)
                    .iter()
                    .any(|parameter| parameter.is_self && parameter.is_mutable)
            {
                return None;
            }
            let (structural, scalar, _) =
                super::super::terminal_unit::calls::mixed_ambient_scalar_graph_signature(
                    program,
                    owner_machine,
                    typed_state,
                    owner_states.first()?.symbol,
                )?;
            Some((structural, scalar))
        };
        // A free machine forwards its per-state structural formals under the
        // ordinary free signature: the same bounded roster, bound on each
        // incoming edge by the whole-parameter or subslice transfer below.
        let free_roster = |owner_machine: &typed_trees::machine::Machine,
                           owner_states: &[typed_trees::state::State],
                           state: &CheckedScalarStateGraph| {
            let typed_state = owner_states
                .iter()
                .find(|entry| entry.symbol == state.state)?;
            if owner_machine.attached_data.is_some() {
                return None;
            }
            let (structural, scalar, _) =
                super::super::terminal_unit::structural_scalar_graph_signature(
                    program,
                    typed_state,
                )?;
            Some((structural, scalar))
        };
        let matches_roster = |owner_machine: &typed_trees::machine::Machine,
                              owner_states: &[typed_trees::state::State],
                              state: &CheckedScalarStateGraph| {
            ambient_roster(owner_machine, owner_states, state)
                .or_else(|| free_roster(owner_machine, owner_states, state))
                .is_some_and(|(structural, scalar)| {
                    state.structural_parameters == structural && state.scalar_parameters == scalar
                })
        };
        let source_states = program.machine_states(source_machine);
        if !(matches_roster(source_machine, source_states, source)
            && matches_roster(target_machine, target_states, target))
        {
            if states.len() != 1 || source.state != target.state {
                return None;
            }
            let (structural, scalar, _) =
                super::super::terminal_unit::structural_scalar_graph_signature(
                    program,
                    source_state,
                )?;
            if source.structural_parameters != structural || source.scalar_parameters != scalar {
                return None;
            }
        }
    }
    let StatementNode::Transition(transition) = program
        .statement_table
        .statements(source_state.statement_nodes)
        .get(successor.statement_ordinal as usize)?
    else {
        return None;
    };
    let destination = if successor.is_continuation {
        transition.continuation
    } else {
        transition.target
    };
    if transition.exit != TransitionExit::Ordinary
        || !program
            .statement_table
            .transition_target_is_valid(destination)
    {
        return None;
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(destination)
    else {
        return None;
    };
    let arguments = program.statement_table.expression_handles(*arguments);
    let target_index = if let Some(target_index) =
        crate::checks::termination::named_transition_target_state_index(
            program,
            machine,
            path.symbol,
        ) {
        if machine.symbol != target_machine.symbol {
            return None;
        }
        target_index
    } else {
        // A cross-machine target spells the reached machine's entry state;
        // `checked_successor` already retained it as the edge's target, so
        // its owning machine must agree with the resolved destination.
        let (entry_machine, _) =
            crate::semantic::calls::find_machine_by_entry_state(program, path.symbol)?;
        if entry_machine.symbol != target_machine.symbol {
            return None;
        }
        0
    };
    // Authored actuals exclude an implicit `self`, and an ambient borrowed
    // receiver owns no graph parameter entry. Pair each actual with its
    // authored formal so ordinals keep the authored target position; an
    // explicit `self` actual has no counterpart here and refuses the edge.
    let target_formals = target_parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| !parameter.is_self)
        .collect::<Vec<_>>();
    if target_states.get(target_index)?.symbol != target.state
        || arguments.len() != successor.argument_count as usize
        || arguments.len() != target_formals.len()
        || target_formals.len()
            != target.scalar_parameters.len()
                + forwarded(target)
                + target.erased_scalar_parameters.len()
                + target.erased_proof_parameters.len()
    {
        return None;
    }
    let proof_only = typed_trees::proof_only::classify(program);
    let mut rows = SuccessorArguments {
        structural: Vec::new(),
        scalar: Vec::new(),
        erased: Vec::new(),
        proof: Vec::new(),
    };
    let mut transferred_affine = Vec::new();
    for (actual, (argument_position, formal)) in
        arguments.iter().zip(target_formals.iter().copied())
    {
        let argument_ordinal = u32::try_from(argument_position).ok()?;
        if formal.relevance.is_erased() {
            let Some(primitive_type) = program.primitive_type_reference(formal.type_reference)
            else {
                // Contract-term erased formals index the contract term lane;
                // the recorded proof term at this edge's exact coordinate is
                // the actual — relowering here would drop scalar leaves.
                let retained = target.erased_proof_parameters.get(rows.proof.len())?;
                if retained.source_position != argument_ordinal
                    || !proof_only.contract_term_carrier(program, formal.type_reference)
                {
                    return None;
                }
                let role = if successor.is_continuation {
                    checked_trees::CheckedProofTermRole::TransitionContinuationArgument {
                        argument_ordinal,
                    }
                } else {
                    checked_trees::CheckedProofTermRole::TransitionArgument { argument_ordinal }
                };
                rows.proof.push(
                    proof_terms
                        .term_at(source.state, successor.statement_ordinal, role)?
                        .clone(),
                );
                continue;
            };
            let target_erased_parameter_index = u32::try_from(rows.erased.len()).ok()?;
            let retained = target.erased_scalar_parameters.get(rows.erased.len())?;
            if retained.source_position != argument_ordinal
                || retained.primitive_type != primitive_type
            {
                return None;
            }
            rows.erased.push(CheckedStructuralScalarArgumentPlan {
                argument_ordinal,
                source: CheckedStructuralScalarArgumentSourcePlan::Expression,
                target_scalar_parameter_index: target_erased_parameter_index,
                primitive_type,
            });
            continue;
        }
        if let Some(primitive_type) = program.primitive_type_reference(formal.type_reference) {
            let target_scalar_parameter_index = u32::try_from(rows.scalar.len()).ok()?;
            let retained = target.scalar_parameters.get(rows.scalar.len())?;
            if retained.source_position != argument_ordinal
                || retained.primitive_type != primitive_type
            {
                return None;
            }
            // Even a direct parameter is read through its exact expression
            // role, preserving current mutable values and computed operands.
            rows.scalar.push(CheckedStructuralScalarArgumentPlan {
                argument_ordinal,
                source: CheckedStructuralScalarArgumentSourcePlan::Expression,
                target_scalar_parameter_index,
                primitive_type,
            });
            continue;
        }
        let target_parameter_index = u32::try_from(rows.structural.len()).ok()?;
        let target_parameter = target.structural_parameters.get(rows.structural.len())?;
        if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(*actual)
            && matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            )
        {
            // An exact builtin range over an established immutable view keeps
            // its source and evaluated endpoints as a subslice transfer,
            // matching the unit edge lane's admission; the replayed endpoint
            // bindings carry the range's builtin operator evidence.
            let subslice = super::super::terminal_unit::calls::view_subslice::admit_replayed(
                program,
                expressions,
                source_machine,
                source_state,
                &source.structural_parameters,
                formal.type_reference,
                *actual,
                successor.statement_ordinal as usize,
                checked_trees::CheckedSubsliceSite::TransitionArgument {
                    argument_ordinal: target_parameter.position,
                },
            )?;
            if subslice.range.type_identity != target_parameter.type_identity {
                return None;
            }
            rows.structural.push(CheckedStructuralControlTransferPlan {
                source: subslice.transfer(),
                target_parameter_index,
            });
            continue;
        }
        let ExpressionNode::Name(name) = program.expression_table.expression(*actual) else {
            return None;
        };
        let [spelling] = program.expression_table.name_path_members(name.members) else {
            return None;
        };
        let (source_position, parameter) =
            source_parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| {
                    name.symbol.is_valid()
                        && name.head_symbol == name.symbol
                        && parameter.symbol == name.symbol
                        && parameter.name == *spelling
                        && !parameter.is_mutable
                        && !parameter.is_const
                        && !parameter.is_self
                })?;
        let source_index = source
            .structural_parameters
            .iter()
            .position(|parameter| parameter.position as usize == source_position)?;
        let source_parameter = &source.structural_parameters[source_index];
        if target_parameter.position != argument_ordinal
            || source_parameter.type_identity != target_parameter.type_identity
            || source_parameter.access != target_parameter.access
            || source_parameter.multiplicity != target_parameter.multiplicity
            || !source_parameter.qualifications.is_empty()
            || !target_parameter.qualifications.is_empty()
            || source_parameter.fused_service_erasure.is_some()
            || target_parameter.fused_service_erasure.is_some()
            || program.normalized_type_identity(parameter.type_reference)
                != program.normalized_type_identity(formal.type_reference)
        {
            return None;
        }
        if source_parameter.access == CheckedStructuralAccess::Owned
            && source_parameter.multiplicity == Multiplicity::Affine
        {
            if transferred_affine.contains(&source_index) {
                return None;
            }
            transferred_affine.push(source_index);
        }
        rows.structural.push(CheckedStructuralControlTransferPlan {
            source: CheckedStructuralControlTransferSourcePlan::Parameter {
                index: u32::try_from(source_index).ok()?,
            },
            target_parameter_index,
        });
    }
    Some(rows)
}

/// Named state transfers share the existing call-occurrence permission ledger.
pub(super) fn owned_transfers(
    program: &TypedTrees,
    machine_symbol: symbols::SymbolHandle,
    state: &State,
    source: &CheckedScalarStateGraph,
    structural: &Arena<CheckedStructuralControlTransferPlan>,
) -> Option<Vec<(PermissionEventSource, facts::PlaceRoot)>> {
    let mut transfers = Vec::new();
    for successor in iter(&source.terminator) {
        let permission_source =
            transition_permission_source(program, machine_symbol, state, successor)?;
        for transfer in structural.span(successor.structural_transfers)? {
            let CheckedStructuralControlTransferSourcePlan::Parameter { index } = transfer.source
            else {
                // Only whole-parameter sources can be owned affine transfers;
                // subslice and projected sources carry no ledger event here.
                continue;
            };
            let parameter = source.structural_parameters.get(index as usize)?;
            if parameter.access != CheckedStructuralAccess::Owned
                || parameter.multiplicity != Multiplicity::Affine
            {
                continue;
            }
            let parameter = program
                .state_parameters(state)
                .get(parameter.position as usize)?;
            transfers.push((
                permission_source,
                facts::PlaceRoot::Symbol(parameter.symbol),
            ));
        }
    }
    Some(transfers)
}

fn transition_permission_source(
    program: &TypedTrees,
    machine_symbol: symbols::SymbolHandle,
    state: &State,
    successor: &CheckedScalarSuccessor,
) -> Option<PermissionEventSource> {
    let machine = crate::lookup::machine_by_symbol(program, machine_symbol)?;
    let statement_index = successor.statement_ordinal as usize;
    let StatementNode::Transition(transition) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?
    else {
        return None;
    };
    let destination = if successor.is_continuation {
        transition.continuation
    } else {
        transition.target
    };
    let mut call_ordinal = 0usize;
    while let Some(site) = crate::semantic::calls::find_call_site(
        program,
        machine_symbol,
        state.symbol,
        statement_index,
        call_ordinal,
    ) {
        if let crate::semantic::calls::CallSite::TransitionNamed { path, .. } = site
            && crate::semantic::calls::transition_call_target(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
            ) == Some(destination)
        {
            return Some(PermissionEventSource::Call {
                statement_index,
                call_ordinal,
                // The ledger retains the authored target, not the normalized
                // entry-state identity used by the executable successor.
                target_symbol: path.symbol,
            });
        }
        call_ordinal = call_ordinal.checked_add(1)?;
    }
    None
}
