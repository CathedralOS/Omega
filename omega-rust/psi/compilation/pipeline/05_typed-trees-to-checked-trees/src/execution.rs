/*
    Executable plans over checked facts: the builders that turn a checked
    program's control, call and ownership facts into the plans a Terminal
    producer consumes, kept separate from temporal flow analysis.

    `finalize_execution` completes the initial plans once checking has
    settled. The four `terminal_*` owners are the builders it calls:
    `terminal_unit` plans Unit machines (calls, control, state graphs,
    cleanup), `terminal_scalar` the scalar
    machine graphs and selections, `terminal_cleanup` the structural control
    cleanup plans over edges and projections, and `terminal_debug` the debug
    metadata plans. `guard_complement` is the one judgment both graph
    builders use to accept a two-guard tail without an authored fallback.
    `citation_erasure` retires fact-only citation calls before any of them
    plans, so no planner sequences a theorem that emits no runtime work.
    `scalar_computations` is this home's other builder: the values pass calls
    it to produce the checked scalar computation plans that `terminal_unit`
    and `terminal_scalar` consume, so one plan builder produces scalar and
    Unit computations.
*/

use arena::Handle;
use checked_trees::{CheckFacts, FlowPermissionEventFact};

pub(crate) mod citation_erasure {
    /*
        Erase fact-only citations before execution planning.

        A statement call citing a theorem-only machine
        (`validation::is_fact_only_citation`) emits no runtime call or work
        ([citation](../../../../../../../wiki/spec/proofs/contracts.md#citation-and-induction)).
        Checking has already discharged its `requires` and imported its `ensures`
        from the call's flow row; that row stays in the arena because borrow
        certificates and contexts hold its handles. Retiring it here is the one
        erasure decision: planners skip retired rows, and statement sequencing
        skips the citation statement itself, so the theorem never enters a unit
        closure. Cited indexed induction judgments remain explicit source-proof
        roots with their source contract and complete assumption closure. Imported
        conclusions do not become execution facts: executable operations still need
        their ordinary Terminal justification rows.
    */
    use crate::flow::indexed_statements;
    use crate::shared::collections::push_unique;
    use checked_trees::CheckFacts;

    /*
        Source roots remain available to checking; execution consumers exclude the
        complete argument computation of these independently admitted citations.
    */
    pub(crate) struct ErasedCitationStatement {
        pub state: symbols::SymbolHandle,
        pub statement_index: usize,
    }

    pub(crate) fn is_fact_only_citation(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        statement: &typed_trees::statement::StatementNode,
    ) -> bool {
        validation::is_fact_only_citation(program, statement, &|machine| {
            facts
                .termination
                .for_machine(machine)
                .map(|plan| plan.checked_summary.clone())
        })
    }

    pub(crate) fn retire_fact_only_citations(
        program: &dyn ::validation::CheckingInput,
        facts: &mut CheckFacts,
    ) -> Vec<ErasedCitationStatement> {
        let mut erased = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (statement_index, statement) in
                    indexed_statements(program, state.statement_nodes)
                {
                    if is_fact_only_citation(program, facts, statement) {
                        erased.push(ErasedCitationStatement {
                            state: state.symbol,
                            statement_index,
                        });
                        if let Some(theorem) = statement.as_call().and_then(|call| {
                            validation::fact_only_theorem(program, call.target_symbol, &|machine| {
                                facts
                                    .termination
                                    .for_machine(machine)
                                    .map(|plan| plan.checked_summary.clone())
                            })
                        }) && validation::mathematical_applications::is_indexed_logical_machine(
                            program, theorem,
                        ) {
                            let citation = checked_trees::CheckedIndexedCitation {
                                caller: machine.symbol,
                                theorem: theorem.symbol,
                            };
                            push_unique(&mut facts.proof.indexed_citations, citation);
                        }
                        facts.flow.control.retire_calls(
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            typed_trees::expression::ExpressionHandle::invalid(),
                            true,
                        );
                    }
                }
            }
        }
        erased
    }
}
pub(crate) mod content_consumption {

    /*
        Retain completed content transactions as ordinary owned value operands.

        The permission producer has already proved and committed each transaction.
        Its fresh payload consumes those parents even when no structural field
        contains them. Publish that roster on the value before Unit disposition
        planning, so a call result and a construction use the same custody rule.
        Terminal lowering follows the exact retained transaction, never a scan that
        could backdate authority onto a captured copy or a later alias.
    */
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use checked_trees::{
        CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan,
    };

    pub(crate) fn attach(
        program: &dyn validation::CheckingInput,
        facts: &mut CheckFacts,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        if facts.qualifications.content.body_transformations.is_empty() {
            return Ok(());
        }
        let nodes = facts
            .values
            .structural_values
            .nodes
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in nodes {
            let (machine, state, statement_index, expression) = {
                let node = facts.values.structural_values.nodes.get(handle);
                (
                    node.machine,
                    node.state,
                    node.statement_index,
                    node.expression,
                )
            };
            let content = &facts.qualifications.content;
            let mut matching = content
                .body_transformations
                .iter()
                .filter(|(_, transaction)| {
                    let completed = facts.flow.control.operands.get(transaction.completed_value);
                    transaction.machine_symbol == machine
                        && transaction.state_symbol == state
                        && completed.statement_index == statement_index as usize
                        && completed.expression == expression
                });
            let Some((transaction_handle, transaction)) = matching.next() else {
                continue;
            };
            if facts
                .values
                .structural_values
                .roots
                .iter()
                .any(|(_, root)| root.root == handle && root.captured_copy.is_valid())
            {
                continue;
            }
            let error = |reason: &str| {
                vec![diagnostics::Diagnostic::error(format!(
                    "cannot retain content consumption in `{}`, state `{}` at statement {}: {reason}",
                    crate::published::labels::symbol_name(program, machine),
                    crate::published::labels::symbol_name(program, state),
                    statement_index,
                ))]
            };
            if matching.next().is_some() {
                return Err(error("completion has ambiguous checked transactions"));
            }
            let mut parents = Vec::new();
            for derived in content
                .body_derivations
                .span_or_empty(transaction.derivations)
            {
                let equation = content.body_conservations.get(derived.conservation);
                for available in content
                    .body_available_inputs
                    .span_or_empty(equation.available_inputs)
                {
                    let place = validation::CanonicalPlace {
                        root: available.root,
                        segments: facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(available.segments)
                            .to_vec(),
                    };
                    let source = match place.root {
                        facts::PlaceRoot::Symbol(symbol) => {
                            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
                        }
                        facts::PlaceRoot::Expression(expression) => {
                            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                value_dependency: None,
                                expression,
                                logical_root_type_identity: None,
                            }
                        }
                        _ => return Err(error("parent has no executable owned source")),
                    };
                    let ProjectedArgumentPath {
                        storage_type: reference,
                        path,
                        ..
                    } = super::terminal_unit::calls::projected_argument_path(
                        program,
                        state,
                        statement_index as usize,
                        &place,
                        Some(machine),
                        None,
                    )
                    .ok_or_else(|| error("parent has no exact typed finite path"))?;
                    let argument = CheckedUnitStructuralArgumentPlan {
                        source,
                        path,
                        projection_evaluations: Default::default(),
                        type_identity: validation::stored_structural_type_identity(
                            program,
                            reference,
                            &[],
                            &[],
                        ),
                        access: CheckedStructuralAccess::Owned,
                    };
                    crate::shared::collections::push_unique(&mut parents, argument);
                }
            }
            if parents.is_empty() {
                return Err(error("transaction has no owned parents"));
            }
            let inputs = facts
                .values
                .structural_values
                .content_inputs
                .insert_many(parents);
            let node = facts.values.structural_values.nodes.get_mut(handle);
            node.content_transformation = transaction_handle;
            node.content_inputs = inputs;
        }
        Ok(())
    }
}
pub(crate) mod finalize_execution {
    use checked_trees::CheckFacts;
    use diagnostics::Diagnostic;

    pub(crate) fn finalize_execution(
        program: &dyn ::validation::CheckingInput,
        mut facts: CheckFacts,
        calling_declarations: &typed_trees::service_declarations::signatures::PreparedBoundarySignatures,
    ) -> Result<CheckFacts, Vec<Diagnostic>> {
        let call_frames = validation::CallFrameResolver::new(program);
        let erased_citations = {
            crate::execution::scalar_computations::record_rank_observations(program, &mut facts);
            facts.flow.terminal_machines =
                crate::execution::terminal_scalar::build_checked_terminal_machine_selections(
                    program,
                    &facts.termination.rank_arrivals,
                );
            crate::execution::citation_erasure::retire_fact_only_citations(program, &mut facts)
        };
        facts.flow.terminal_structural_control_cleanups =
            crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
                program, &facts,
            );
        let mut cleanup_diagnostics = Vec::new();
        let boundary_signature_reaches = program
            .traits()
            .iter()
            .flat_map(|owner| program.executable_trait_machine_instances(owner))
            .filter_map(|signature| {
                validation::trait_signature_service_reach(program, signature.symbol).map(
                    |services| {
                        (
                            signature.symbol,
                            facts.service_reaches.rows.intern(services),
                        )
                    },
                )
            })
            .collect::<Vec<_>>();
        let (mut terminal_unit_effects, partial_cleanups, prepared_calls) =
            crate::execution::terminal_unit::build_checked_unit_effect_plans(
                program,
                &facts,
                calling_declarations,
                &boundary_signature_reaches,
                crate::execution::terminal_unit::ScalarCalleeClosure,
                call_frames.as_ref(),
            );
        crate::__mem_probe("fin-after-unit-effects");
        facts.flow.terminal_partial_affine_unit_cleanups = partial_cleanups;
        let (nominal_cleanups, nominal_types) =
            crate::execution::terminal_unit::build_checked_nominal_affine_unit_cleanup_plans(
                program,
                &facts,
                &terminal_unit_effects,
                &mut cleanup_diagnostics,
            );
        facts.flow.terminal_nominal_affine_unit_cleanups = nominal_cleanups;
        if let Err((identity, reason)) = terminal_unit_effects.adopt_structural_types(nominal_types)
        {
            cleanup_diagnostics.push(Diagnostic::error(format!(
                "Unit cleanup plans register structural type `{}` with a shape the ordinary Unit roster does not carry: {reason}",
                program.type_identity_text(identity).unwrap_or_default()
            )));
        }
        crate::__mem_probe("fin-after-nominal");
        let mut construction_shapes = Vec::new();
        for state in terminal_unit_effects
            .machines
            .iter()
            .chain(
                facts
                    .flow
                    .terminal_partial_affine_unit_cleanups
                    .machines
                    .iter()
                    .map(|plan| &plan.machine),
            )
            .map(|plan| plan.state)
            .chain(
                terminal_unit_effects
                    .composed_machines
                    .iter()
                    .flat_map(|plan| plan.states.iter().map(|state| state.state)),
            )
        {
            crate::execution::terminal_scalar::constructions::retain_shapes(
                program,
                &facts.values.scalar_computations,
                &facts.values.structural_values,
                state,
                |statement| {
                    !erased_citations.iter().any(|erased| {
                        erased.state == state && erased.statement_index == statement as usize
                    })
                },
                &mut construction_shapes,
            )
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "cannot retain constructed value storage shapes for state `{}` of machine `{}`: {reason}",
                    program.symbols.name(state),
                    validation::find_state(program, state)
                        .map(|(machine, _)| machine.name.as_str())
                        .unwrap_or("<unknown>"),
                ))]
            })?;
        }
        if let Err((identity, reason)) =
            terminal_unit_effects.adopt_structural_types(construction_shapes)
        {
            cleanup_diagnostics.push(Diagnostic::error(format!(
                "scalar computations construct structural type `{}` with a shape the ordinary Unit roster does not carry: {reason}",
                program.type_identity_text(identity).unwrap_or_default()
            )));
        }
        if !cleanup_diagnostics.is_empty() {
            return Err(cleanup_diagnostics);
        }
        crate::__mem_probe("fin-after-constructions");
        facts.flow.terminal_unit_effects = terminal_unit_effects;
        crate::execution::terminal_unit::retain_structural_value_calls(&mut facts, prepared_calls)
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "cannot retain structural value calls: {reason}"
                ))]
            })?;

        crate::__mem_probe("fin-before-completion");
        crate::execution::terminal_unit::cleanup::finalize_completion_actions(program, &mut facts)
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "cannot plan contextual call cleanup against the executable hook roster: {reason}"
                ))]
            })?;

        Ok(facts)
    }
}
pub(crate) mod guard_complement {
    /*
        Whether an authored two-guard tail needs no fallback because its second
        guard holds exactly where the first fails. Unit planning and the guarded
        exit plans both ask this of the selected Guard rows, through the one
        predicate in `checked_trees::values::guard_complement`, so both admit the
        same guard pairs.
    */
    use checked_trees::{
        CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionPlans,
        CheckedScalarExpressionRole, CheckedStructuralParameterField,
    };

    /*
        The Guard row at `ordinal + 1` is the exact complement of the row at
        `ordinal`, both in `state`.
    */
    pub(crate) fn complementary(
        program: &dyn ::validation::CheckingInput,
        expressions: &CheckedScalarExpressionPlans,
        state: &typed_trees::state::State,
        ordinal: u32,
    ) -> bool {
        let Some((first, second)) = guard_pair(expressions, state.symbol, ordinal) else {
            return false;
        };
        checked_trees::values::guard_complement::exact_complement(first, second, |subject| {
            declared_cases(program, state, subject)
        })
    }

    /* Exactly one Boolean Guard row at each of two consecutive statements. */
    fn guard_pair(
        expressions: &CheckedScalarExpressionPlans,
        state: symbols::SymbolHandle,
        ordinal: u32,
    ) -> Option<(&CheckedBooleanExpression, &CheckedBooleanExpression)> {
        let guard = |ordinal| {
            let mut selected = expressions.expressions.iter().filter(|expression| {
                expression.state == state
                    && expression.statement_ordinal == ordinal
                    && expression.role == CheckedScalarExpressionRole::Guard
            });
            let expression = selected.next()?;
            if selected.next().is_some() {
                return None;
            }
            match &expression.expression {
                CheckedScalarExpression::Boolean(expression) => Some(expression.as_ref()),
                _ => None,
            }
        };
        Some((guard(ordinal)?, guard(ordinal.checked_add(1)?)?))
    }

    /*
        The declared case keys of the sum the subject path reaches from its
        authored state parameter, or `None` unless every member of that sum is a
        case.
    */
    fn declared_cases(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        subject: &CheckedStructuralParameterField,
    ) -> Option<Vec<checked_trees::CheckedMemberIdentity>> {
        let (_, _, sum, _) = crate::values::resolve_structural_parameter_path(
            program,
            program.state_parameters(state),
            subject.parameter_position,
            &subject.path,
        )?;
        let sum = crate::values::structural_data(program, sum)?;
        program
            .data_members(sum)
            .iter()
            .map(|member| match member {
                typed_trees::data::DataMember::Variant(case) => {
                    Some(checked_trees::CheckedMemberIdentity::of_member(
                        case.identity,
                        case.name.clone(),
                    ))
                }
                typed_trees::data::DataMember::Field(_) => None,
            })
            .collect()
    }
}
#[path = "execution_scalar_computations.rs"]
pub(crate) mod scalar_computations;
pub(super) mod selected_transitions {
    /*
        Executable selection over canonical transition coordinates. Source formation
        retains every arm; a closed static dispatch plans only its admitted outcome.
    */
    use typed_trees::statement::{StatementNode, TableTransition, TransitionGuardNode};

    pub(super) struct SelectedTransition<'source> {
        pub statement_ordinal: u32,
        pub transition: &'source TableTransition,
    }

    /*
        Resolve a complete static prefix, never speculate past an unknown guard.
        Guards read here are closed static Boolean values, so omitting their runtime
        evaluation removes no call, write, loan, or trapping operand. Earlier local
        initializers still belong to the ordinary statement sequence.
    */
    pub(super) fn selected_transition<'source>(
        program: &dyn validation::CheckingInput,
        statements: &'source [StatementNode],
        start: usize,
    ) -> Option<SelectedTransition<'source>> {
        for (index, statement) in statements.iter().enumerate().skip(start) {
            let transition = statement.as_transition()?;
            if transition.continuation.is_valid() {
                return None;
            }
            let selected = match transition.guard {
                TransitionGuardNode::Always => true,
                TransitionGuardNode::When(guard) => program.static_boolean_value(guard)?,
            };
            if !selected {
                continue;
            }
            if program.static_transition_type_context_is_unreachable(transition.target) {
                return None;
            }
            return Some(SelectedTransition {
                statement_ordinal: u32::try_from(index).ok()?,
                transition,
            });
        }
        None
    }
}
pub(crate) mod successor_completion {
    /*
        Retain the source invocation boundary when scalar or composed control fuses
        it into an edge. Unknown completion stays potentially returning; only the
        resolved callable's checked topology can suppress normal caller cleanup.
    */
    use checked_trees::CheckedSuccessorCompletion;

    pub(super) fn classify(
        program: &dyn validation::CheckingInput,
        source: &typed_trees::machine::Machine,
        target: symbols::SymbolHandle,
    ) -> CheckedSuccessorCompletion {
        let Some((callable, _)) = validation::terminal_callable_target(program, source, target)
        else {
            return CheckedSuccessorCompletion::StateArrival;
        };
        if matches!(
            validation::boundary_requirement_completion(program, callable),
            semantic_vocabulary::BoundaryCompletion::External(_)
        ) || validation::call_target_state(program, callable).is_some_and(|state| {
            validation::normal_completion_possible(program, state) == Some(false)
        }) {
            CheckedSuccessorCompletion::NoNormalInvocation
        } else {
            CheckedSuccessorCompletion::ReturningInvocation
        }
    }
}
pub(crate) mod terminal_cleanup {
    /*
        Whole-root cleanup for the older structural-control consumers, plus lexical
        result identity for composed control. Composed execution gets its ordered
        whole/residual/nominal actions from `terminal_unit::cleanup::normal_edges`.
        Do not rebuild a parallel projected-edge table here: it has no executable
        consumer, and requiring it before normal-edge planning prevents partial
        transfers from reaching the operation that already owns their cleanup.
    */
    use std::collections::BTreeSet;

    use checked_trees::{
        CheckFacts, CheckedStructuralControlCleanupPlans, CheckedStructuralControlEdgeCleanupPlan,
        CheckedStructuralControlStateCleanupPlan,
    };
    use language_semantics::{
        Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
        PermissionEventSource, PermissionProvenance,
    };
    use typed_trees::statement::{
        StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
    };

    use crate::execution::permission_events;
    use crate::flow::FlowOwnershipEventSource;

    /*
        Retain the first source-handle-free structural control-edge cleanup slice.

        The permission ledger is authoritative for which whole affine parameters
        are eligible for no-code state-exit disposal. Per-arm move discovery is the
        same checked ownership input used by multiplicity validation. Anything that
        needs projected, nominal, or claim-bearing cleanup omits the complete
        state plan rather than publishing partial evidence. Exact local result
        obligations stay separate: ordinary completion disposes the local, while
        a named successor must account for its selected transfer or disposal.
    */
    pub(crate) fn build_checked_structural_control_cleanup_plans(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
    ) -> CheckedStructuralControlCleanupPlans {
        let mut states = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                if let Some(plan) = build_state_plan(program, facts, machine, state) {
                    states.push(plan);
                }
            }
        }
        CheckedStructuralControlCleanupPlans { states }
    }

    fn build_state_plan(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> Option<CheckedStructuralControlStateCleanupPlan> {
        let statements = program.statement_table.statements(state.statement_nodes);
        let has_structural_control = statements.iter().any(|statement| {
            let Some(transition) = statement.as_transition() else {
                return false;
            };
            transition.exit == TransitionExit::Ordinary
                && matches!(
                    program.statement_table.transition_target(transition.target),
                    TransitionTargetNode::Named { path, .. }
                        if validation::activation_target_state_index(
                            program, machine, path.symbol,
                        ).is_some()
                            || validation::find_machine_by_entry_state(
                                program,
                                path.symbol,
                            )
                            .is_some()
                )
        });
        if !has_structural_control {
            return None;
        }

        let parameters = program.state_parameters(state);
        let result_locals = state_exit_result_locals(program, facts, machine, state)?;
        let discard_parameters = checked_whole_affine_discard_parameters_excluding_results(
            program,
            facts,
            machine.symbol,
            state,
            &result_locals,
        )?;

        let flow = facts.flow.control.state(machine.symbol, state.symbol)?;
        let segments = &facts.flow.ownership.segments;
        let moves = facts
            .flow
            .ownership
            .storage_transfers
            .span_or_empty(flow.storage_transfers);
        let mut edges = Vec::new();
        for (statement_index, statement) in statements.iter().enumerate() {
            let Some(transition) = statement.as_transition() else {
                continue;
            };
            if transition.exit != TransitionExit::Ordinary {
                continue;
            }
            let TransitionTargetNode::Named { path, .. } =
                program.statement_table.transition_target(transition.target)
            else {
                continue;
            };
            let target =
                match validation::activation_target_state_index(program, machine, path.symbol) {
                    Some(target_index) => &program.machine_states(machine)[target_index],
                    None => match validation::find_machine_by_entry_state(program, path.symbol) {
                        Some((_, entry)) => entry,
                        None => continue,
                    },
                };

            let mut transferred = BTreeSet::new();
            for event in moves.iter().filter(|event| {
                matches!(
                    event.source,
                    FlowOwnershipEventSource::Statement {
                        statement_index: source_index,
                    } if source_index == statement_index
                ) || matches!(
                    event.source,
                    FlowOwnershipEventSource::Call {
                        statement_index: source_index,
                        target_symbol,
                        ..
                    } if source_index == statement_index && target_symbol == validation::call_target_state(program, path.symbol)
                        .map_or(path.symbol, |state| state.symbol)
                )
            }) {
                let Some(root) = event.root.as_symbol() else {
                    continue;
                };
                let Some((_, position)) = discard_parameters
                    .iter()
                    .find(|(candidate, _)| *candidate == root)
                else {
                    continue;
                };
                if !segments.span_or_empty(event.segments).is_empty() {
                    projected_read_retains_parent(
                        program,
                        state,
                        statement_index,
                        transition,
                        root,
                        segments.span_or_empty(event.segments),
                    )?;
                    continue;
                }
                transferred.insert(*position);
            }
            for (symbol, position) in &discard_parameters {
                let operand_moves = validation::transition_operand_moves(
                    program,
                    facts,
                    machine.symbol,
                    state,
                    statement_index,
                    *symbol,
                )?;
                if operand_moves.len() > 1
                    || (!operand_moves.is_empty() && transferred.contains(position))
                {
                    return None;
                }
                for source in operand_moves {
                    let mut transfers = permission_events(facts).filter(|(_, event)| {
                        event.machine_symbol == machine.symbol
                            && event.state_symbol == state.symbol
                            && event.root == facts::PlaceRoot::Symbol(*symbol)
                            && event.kind == PermissionEventKind::Transfer
                            && event.source == source
                    });
                    let (_, transfer) = transfers.next()?;
                    if transfers.next().is_some()
                        || transfer.access != PermissionAccess::Owned
                        || transfer.multiplicity != Multiplicity::Affine
                        || transfer.claim_identity != PermissionClaimIdentity::Unknown
                        || transfer.provenance != PermissionProvenance::Unknown
                        || transfer.obligation_live
                        || !facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(transfer.segments)
                            .is_empty()
                    {
                        return None;
                    }
                    transferred.insert(*position);
                }
            }
            let trivial_affine_discard_parameter_positions = discard_parameters
                .iter()
                .filter_map(|(symbol, position)| {
                    (!transferred.contains(position)).then_some((*symbol, *position))
                })
                .map(|(symbol, position)| {
                    let parameter = parameters
                        .iter()
                        .find(|parameter| parameter.symbol == symbol)?;
                    (!super::terminal_unit::types::type_graph_requires_nominal_drop(
                        program,
                        parameter.type_reference,
                        &[],
                        &mut std::collections::BTreeSet::new(),
                    ))
                    .then_some(position)
                })
                .collect::<Option<Vec<_>>>()?;
            edges.push(CheckedStructuralControlEdgeCleanupPlan {
                statement_ordinal: u32::try_from(statement_index).ok()?,
                target_state: target.symbol,
                trivial_affine_discard_parameter_positions,
            });
        }
        (!edges.is_empty()).then_some(CheckedStructuralControlStateCleanupPlan {
            machine: machine.symbol,
            state: state.symbol,
            edges,
        })
    }

    /*
        Whether a projected read out of `root` leaves the parent's storage intact,
        so the parent keeps its ordinary state-exit discard row.

        Reading a copyable subtree is an observation: the read materializes a
        fresh value and the parent survives it, at any field or index path. An
        affine subtree is different — that read is a partial move, and this lexical
        roster does not enumerate its complement. Selected-edge emission retains
        original projected custody and completes that complement separately. Crossing
        a case also needs that proof, because an unproven case path names storage
        the edge never selected. So the selection proof is required exactly where
        it is load-bearing rather than on every projected read, which is what made
        a copyable field or element read look like an unaccountable partial move.
        Claims and nominal drops still cannot use the no-code retirement.
    */
    fn projected_read_retains_parent(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        transition: &typed_trees::statement::TableTransition,
        root: symbols::SymbolHandle,
        segments: &[facts::PlaceSegment],
    ) -> Option<()> {
        if segments.iter().any(
            |segment| matches!(segment, facts::PlaceSegment::Field { symbol } if !symbol.is_valid()),
        ) {
            return Some(());
        }
        let parameter = program.state_parameter_by_symbol(state, root)?;
        let payload = validation::project_type_reference_from_segments(
            program,
            parameter.type_reference,
            segments,
        )?;
        let multiplicity = program.type_multiplicity(payload);
        if !matches!(
            multiplicity,
            Multiplicity::Affine | Multiplicity::Unrestricted
        ) || !(validation::has_contents(
            program,
            payload,
            &[],
            validation::ContentsRequirement::NumericOwned,
        ) || validation::has_contents(
            program,
            payload,
            &[],
            validation::ContentsRequirement::OwnedOrSharedView,
        )) || super::terminal_unit::types::type_graph_requires_nominal_drop(
            program,
            payload,
            &[],
            &mut std::collections::BTreeSet::new(),
        ) {
            return None;
        }
        let crosses_case = segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }));
        if multiplicity == Multiplicity::Unrestricted && !crosses_case {
            return Some(());
        }
        let TransitionGuardNode::When(guard) = transition.guard else {
            return None;
        };
        let (subject, selected_case) =
            crate::published::proof::exact_outcome_case_test(program, guard)?;
        let subject = validation::contextual_canonical_place_from_expression(
            program,
            state.symbol,
            statement_index,
            subject,
        )?;
        if subject.root != facts::PlaceRoot::Symbol(root) || !subject.segments.is_empty() {
            return None;
        }
        let [
            facts::PlaceSegment::Case { variant },
            facts::PlaceSegment::Field { symbol },
        ] = segments
        else {
            return None;
        };
        if *variant != selected_case || !program.data_definitions().iter().any(|definition| {
            program.data_members(definition).iter().any(|member| {
                matches!(member, typed_trees::data::DataMember::Variant(variant)
                    if variant.symbol == selected_case && program.data_payload_fields(variant).iter().any(|field| field.symbol == *symbol))
            })
        }) {
            return None;
        }
        Some(())
    }

    fn checked_whole_affine_discard_parameters_excluding_results(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: symbols::SymbolHandle,
        state: &typed_trees::state::State,
        result_locals: &[symbols::SymbolHandle],
    ) -> Option<Vec<(symbols::SymbolHandle, u32)>> {
        let parameters = program.state_parameters(state);
        let entry_claim_roots = permission_events(facts)
            .filter_map(|(_, event)| {
                (event.machine_symbol == machine
                    && event.state_symbol == state.symbol
                    && event.source == PermissionEventSource::StateEntry
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned)
                    .then_some(event.root)
            })
            .collect::<Vec<_>>();
        let selection_source_roots = facts
            .flow
            .ownership
            .owned_selections
            .iter()
            .filter(|(_, receipt)| receipt.machine == machine && receipt.state == state.symbol)
            .flat_map(|(_, receipt)| {
                facts
                    .flow
                    .ownership
                    .selection_sources
                    .span_or_empty(receipt.sources)
                    .iter()
                    .map(|source| facts::PlaceRoot::Symbol(source.symbol))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let statements = program.statement_table.statements(state.statement_nodes);
        let mid_body_move = |source: PermissionEventSource| -> bool {
            match source {
                PermissionEventSource::Statement { statement_index } => !matches!(
                    statements.get(statement_index),
                    Some(StatementNode::Transition(_))
                ),
                PermissionEventSource::Call {
                    statement_index,
                    target_symbol,
                    ..
                } => {
                    let edge_target = match statements.get(statement_index) {
                        Some(StatementNode::Transition(transition))
                            if transition.exit == TransitionExit::Ordinary =>
                        {
                            match program.statement_table.transition_target(transition.target) {
                                TransitionTargetNode::Named { path, .. } => Some(path.symbol),
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    edge_target != Some(target_symbol) && program.machine(target_symbol).is_some()
                }
                _ => false,
            }
        };
        let moved_roots = permission_events(facts)
            .filter_map(|(_, event)| {
                (event.machine_symbol == machine
                    && event.state_symbol == state.symbol
                    && matches!(
                        event.kind,
                        PermissionEventKind::Transfer | PermissionEventKind::Consume
                    )
                    && event.access == PermissionAccess::Owned
                    && mid_body_move(event.source)
                    && facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .is_empty())
                .then_some(event.root)
            })
            .collect::<Vec<_>>();
        let expected_discard_parameters = parameters
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(position, parameter)| {
                if (crate::checks::type_multiplicity(program, parameter.type_reference)
                    != Multiplicity::Affine)
                    || entry_claim_roots.contains(&facts::PlaceRoot::Symbol(parameter.symbol))
                    || selection_source_roots.contains(&facts::PlaceRoot::Symbol(parameter.symbol))
                    || moved_roots.contains(&facts::PlaceRoot::Symbol(parameter.symbol))
                {
                    return None;
                }
                Some((parameter.symbol, u32::try_from(position).ok()?))
            })
            .collect::<Vec<_>>();
        let mut discard_parameters = Vec::new();
        for event in facts
            .flow
            .ownership
            .state_exit_cleanup_roots(machine, state.symbol)?
        {
            if matches!(event.root, facts::PlaceRoot::Symbol(symbol) if result_locals.contains(&symbol))
            {
                continue;
            }
            if event.access != PermissionAccess::Owned
                || event.multiplicity != Multiplicity::Affine
                || event.obligation_live
                || event.claim_identity != PermissionClaimIdentity::Unknown
                || event.provenance != PermissionProvenance::Unknown
                || !facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty()
            {
                return None;
            }
            let root = event.root.as_symbol()?;
            let position = parameters.iter().position(|parameter| {
                (parameter.symbol == root)
                    && crate::checks::type_multiplicity(program, parameter.type_reference)
                        == Multiplicity::Affine
            })?;
            let position = u32::try_from(position).ok()?;
            if discard_parameters.contains(&(root, position)) {
                return None;
            }
            discard_parameters.push((root, position));
        }
        discard_parameters.sort_unstable_by_key(|(_, position)| std::cmp::Reverse(*position));
        if discard_parameters != expected_discard_parameters {
            return None;
        }
        Some(discard_parameters)
    }

    /*
        Separate exact local result obligations from the parameter cleanup roster.
        The graph producer joins candidate cleanup roots to actual result bindings;
        selected normal-edge actions own the whole or partial disposal. This roster
        does not turn local results into parameters or reconstruct outgoing syntax.
    */
    pub(super) fn state_exit_result_locals(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> Option<Vec<symbols::SymbolHandle>> {
        let parameters = program.state_parameters(state);
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut results = Vec::new();
        for drop in facts
            .flow
            .ownership
            .state_exit_cleanup_roots(machine.symbol, state.symbol)?
            .into_iter()
            .chain(permission_events(facts).filter_map(|(_, event)| {
                (event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.source == PermissionEventSource::StateExit
                    && event.kind == PermissionEventKind::Consume
                    && event.multiplicity == Multiplicity::Linear
                    && event.access == PermissionAccess::Owned
                    && event.claim_identity != PermissionClaimIdentity::Unknown
                    && event.provenance != PermissionProvenance::Unknown
                    && !event.obligation_live
                    && facts
                        .contextual_cleanup
                        .dispositions
                        .iter()
                        .any(|(_, disposition)| {
                            let evaluation = match disposition.exit.kind {
                                checked_trees::FlowClaimJoinExitKind::TransitionTarget
                                | checked_trees::FlowClaimJoinExitKind::TransitionContinuation => {
                                    let Some(StatementNode::Transition(transition)) =
                                        statements.get(disposition.exit.statement_index)
                                    else {
                                        return false;
                                    };
                                    checked_trees::FlowPermissionEvaluation::TransitionTarget(
                                        if disposition.exit.kind
                                            == checked_trees::FlowClaimJoinExitKind::TransitionTarget
                                        {
                                            transition.target
                                        } else {
                                            transition.continuation
                                        },
                                    )
                                }
                                _ => checked_trees::FlowPermissionEvaluation::Statement,
                            };
                            disposition.machine == machine.symbol
                                && disposition.state == state.symbol
                                && disposition.exit.state_symbol == state.symbol
                                && disposition.source == PermissionEventSource::StateExit
                                && event.evaluation == evaluation
                                && disposition.root == event.root
                                && facts
                                    .flow
                                    .ownership
                                    .segments
                                    .span_or_empty(disposition.path)
                                    == facts.flow.ownership.segments.span_or_empty(event.segments)
                        }))
                .then_some(event)
            }))
            .chain(
                facts
                    .flow
                    .ownership
                    .selected_cleanup_permissions
                    .iter()
                    .filter_map(|(_, event)| {
                        (event.machine_symbol == machine.symbol && event.state_symbol == state.symbol)
                            .then_some(event)
                    }),
            )
        {
            let Some(symbol) = drop.root.as_symbol() else {
                continue;
            };
            if parameters
                .iter()
                .any(|parameter| parameter.symbol == symbol)
            {
                continue;
            }
            if !matches!(
                drop.multiplicity,
                Multiplicity::Affine | Multiplicity::Linear
            ) || drop.access != PermissionAccess::Owned
                || (drop.multiplicity == Multiplicity::Affine
                    && drop.claim_identity != PermissionClaimIdentity::Unknown)
                || drop.obligation_live
            {
                return None;
            }
            if results.contains(&symbol) {
                continue;
            }
            let (statement_index, local) = program.state_local_declaration(state, symbol)?;
            if local.symbol != symbol {
                return None;
            }
            if statements[..=statement_index]
                .iter()
                .any(|statement| matches!(statement, StatementNode::Transition(_)))
            {
                return None;
            }
            results.push(symbol);
        }
        Some(results)
    }
}
pub(crate) mod terminal_debug {
    use checked_trees::{
        CheckedTerminalDebugPlans, CheckedTerminalMachineDebugPlan, CheckedTerminalStateDebugPlan,
    };
    use typed_trees::{
        domain::ProofFact,
        expression::{ExpressionHandle, ExpressionNode},
        signature::SignatureContractKind,
        statement::{StatementNode, TransitionGuardNode, TransitionTargetNode},
    };

    pub(crate) fn build_checked_terminal_debug_plans(
        program: &dyn ::validation::CheckingInput,
    ) -> CheckedTerminalDebugPlans {
        CheckedTerminalDebugPlans {
            machines: program
                .machines()
                .iter()
                .map(|machine| build_machine_debug_plan(program, machine))
                .collect(),
        }
    }

    fn build_machine_debug_plan(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> CheckedTerminalMachineDebugPlan {
        let machine_span = program.symbols.symbol_source_span(machine.symbol);
        let contract_span = source_ensures_span(program, machine)
            .filter(|span| *span != source::SourceSpan::default())
            .filter(|span| program.symbols.source_file(*span).is_some())
            .or(machine_span);
        let states = program
            .machine_states(machine)
            .iter()
            .map(|state| CheckedTerminalStateDebugPlan {
                state: state.symbol,
                state_span: program.symbols.symbol_source_span(state.symbol),
                parameter_spans: program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| {
                        program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                    })
                    .map(|parameter| program.symbols.symbol_source_span(parameter.symbol))
                    .collect(),
                transition_spans: source_transition_spans(program, state),
                operation_spans: source_operation_spans(program, state),
            })
            .collect::<Vec<_>>();
        let mut source_ids = machine_span
            .into_iter()
            .chain(contract_span)
            .chain(states.iter().flat_map(|state| {
                state
                    .state_span
                    .into_iter()
                    .chain(state.parameter_spans.iter().flatten().copied())
                    .chain(state.transition_spans.iter().copied())
                    .chain(state.operation_spans.iter().copied())
            }))
            .map(|span| span.source_id.0)
            .collect::<Vec<_>>();
        source_ids.sort_unstable();
        source_ids.dedup();
        let source_files = source_ids
            .into_iter()
            .filter_map(|source_id| {
                program.symbols.source_file(source::SourceSpan::new(
                    source::SourceId(source_id),
                    source::Span::default(),
                ))
            })
            .cloned()
            .collect();
        CheckedTerminalMachineDebugPlan {
            machine: machine.symbol,
            machine_span,
            contract_span,
            states,
            source_files,
        }
    }

    fn source_ensures_span(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Option<source::SourceSpan> {
        let contract = program
            .machine_contracts(machine)
            .iter()
            .find(|contract| contract.kind == SignatureContractKind::Ensures)?;
        let [ProofFact::Expression(expression)] = program.proof_facts.span_or_empty(contract.facts)
        else {
            return None;
        };
        Some(program.expression_table.source_span(*expression))
    }

    fn source_transition_spans(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
    ) -> Vec<source::SourceSpan> {
        crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::Transition(transition) => Some(transition.source_span),
                StatementNode::Expression(expression) => {
                    Some(program.expression_table.source_span(*expression))
                }
                _ => None,
            })
            .collect()
    }

    fn source_operation_spans(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
    ) -> Vec<source::SourceSpan> {
        let mut spans = Vec::new();
        for statement in program.statement_table.statements(state.statement_nodes) {
            match statement {
                StatementNode::Expression(expression) => {
                    collect_source_operation_spans(program, *expression, &mut spans);
                }
                StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                    collect_source_operation_spans(program, local.initial_value, &mut spans);
                }
                StatementNode::Transition(transition) => {
                    if let TransitionGuardNode::When(guard) = transition.guard {
                        collect_source_operation_spans(program, guard, &mut spans);
                    }
                    if let TransitionTargetNode::Named { arguments, .. } =
                        program.statement_table.transition_target(transition.target)
                    {
                        for expression in program.statement_table.expression_handles(*arguments) {
                            collect_source_operation_spans(program, *expression, &mut spans);
                        }
                    }
                }
                _ => {}
            }
        }
        spans
    }

    fn collect_source_operation_spans(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        spans: &mut Vec<source::SourceSpan>,
    ) {
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {
                spans.push(program.expression_table.source_span(expression));
            }
            ExpressionNode::Binary(binary) => {
                collect_source_operation_spans(program, binary.left, spans);
                collect_source_operation_spans(program, binary.right, spans);
                spans.push(program.expression_table.source_span(expression));
            }
            ExpressionNode::Unary(unary) => {
                collect_source_operation_spans(program, unary.operand, spans);
                spans.push(program.expression_table.source_span(expression));
            }
            _ => {}
        }
    }
}
pub(crate) mod terminal_scalar {
    /*
        Checked scalar state facts that every Unit body shares: a state's
        scalar terminator, every state's guarded exits, the
        record shapes scalar computations construct inline, and each machine's
        signature eligibility in `CheckedTerminalMachineSelections`.

        - While checked facts are built, `facts::build_check_facts` calls
          `build_checked_guarded_exit_plans`, which collects the ordered guarded
          exits and fallback of every state of every machine (`guarded_exits`),
          and `build_checked_terminal_machine_selections`.
        - `execution::terminal_unit` plans each state's exit with
          `checked_terminator`; its statements, locals included, belong to the
          state's statement sequence.
        - `execution::finalize_execution` adopts the shapes a computation
          constructs inline (`constructions::retain_shapes`) into the Unit type
          roster, so a record built only inside an expression still has its
          declaration.

        There is no separate scalar graph: Unit bodies are the one lowering route
        for scalar machines, and these facts are what they read.
    */
    use crate::execution::guard_complement::complementary;
    use crate::execution::selected_transitions::selected_transition;
    use crate::execution::successor_completion::classify;
    use crate::execution::terminal_unit::state_graph::natural_rank_plans;
    use checked_trees::{
        CheckedScalarBranchDestination, CheckedScalarStateTerminator,
        CheckedStructuralControlSuccessorPlan, CheckedTerminalMachineSelection,
        CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
    };

    pub(crate) fn build_checked_terminal_machine_selections(
        program: &dyn ::validation::CheckingInput,
        rank_arrivals: &checked_trees::CheckedRankArrivalPlan,
    ) -> CheckedTerminalMachineSelections {
        CheckedTerminalMachineSelections {
            machines: program
                .machines()
                .iter()
                .map(|machine| CheckedTerminalMachineSelection {
                    machine: machine.symbol,
                    name: program.symbols.display_path(machine.symbol, "::"),
                    signature: if !machine.type_parameters.is_empty()
                        || !machine.owned_data.is_empty()
                        || (machine.termination_plan.implementation_witness.is_some()
                            && natural_rank_plans(program, machine, rank_arrivals).is_none())
                        || !machine.supplies_checked_body()
                    {
                        CheckedTerminalSignatureEligibility::Unsupported
                    } else if !program
                        .service_reach_rows
                        .services(machine.service_reach_row)
                        .is_empty()
                        || !machine.invokes.is_empty()
                    {
                        CheckedTerminalSignatureEligibility::FreeUnitEffect
                    } else {
                        CheckedTerminalSignatureEligibility::Eligible
                    },
                })
                .collect(),
        }
    }
    use typed_trees::statement::{
        StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
    };

    pub(crate) mod constructions {
        //! Structural values retain a type namespace without inventing source parameters.

        use super::super::terminal_unit::reference_backing_shapes;
        use super::super::terminal_unit::scalar_case_value_shapes;
        use super::super::terminal_unit::scalar_operand_shapes;
        use checked_trees::{
            CheckedScalarComputationKind as ComputationKind, CheckedScalarComputationPlans,
            CheckedUnitStructuralTypePlan as TypePlan,
        };
        use symbols::SymbolHandle;
        use typed_trees::types::TypeReferenceHandle;

        /// Retain a Case subject's value shapes and enqueue its field values.
        fn retain_case_subject_fields(
            shapes: &mut Vec<TypePlan>,
            plans: &CheckedScalarComputationPlans,
            program: &dyn ::validation::CheckingInput,
            subject: &checked_trees::CheckedScalarCaseConstruction,
            pending: &mut Vec<arena::Handle<checked_trees::CheckedScalarComputation>>,
        ) -> Result<(), String> {
            retain_declarations(
                program,
                shapes,
                scalar_case_value_shapes(program, subject.type_reference).ok_or_else(|| {
                    format!(
                        "cannot collect case subject storage for {}",
                        type_description(program, subject.type_reference)
                    )
                })?,
            )?;
            pending.extend(
                plans
                    .case_fields
                    .span(subject.fields)
                    .ok_or_else(|| {
                        format!(
                            "case subject {} has no retained field span",
                            type_description(program, subject.type_reference)
                        )
                    })?
                    .iter()
                    .map(|field| field.value),
            );
            Ok(())
        }

        pub(crate) fn retain_shapes(
            program: &dyn ::validation::CheckingInput,
            plans: &CheckedScalarComputationPlans,
            structural_values: &checked_trees::CheckedStructuralValuePlans,
            state: SymbolHandle,
            is_executable: impl Fn(u32) -> bool,
            shapes: &mut Vec<TypePlan>,
        ) -> Result<(), String> {
            (|| {
                macro_rules! retain_operand {
                    ($type_reference:expr, $context:expr) => {{
                        let reference = $type_reference;
                        let declarations =
                            scalar_operand_shapes(program, reference).ok_or_else(|| {
                                format!(
                                    "{}: cannot collect operand storage for {}",
                                    $context,
                                    type_description(program, reference)
                                )
                            })?;
                        retain_declarations(program, shapes, declarations)
                            .map_err(|reason| format!("{}: {reason}", $context))
                    }};
                }
                // A stored reference publishes only its pointer shape. Forming one also
                // demands actual backing storage, whether named or computed. Follow the
                // retained formation/value edges, never transitive pointee types or
                // speculative arena nodes; recursive pointer catalogs stay finite.
                let mut retained_values = Vec::new();
                for root in structural_values.roots_for_state(state) {
                    // Erased argument roots retain proof/custody identity, but own no
                    // executable constructor storage. Live dependencies still follow
                    // their complete retained value graph below.
                    if !is_executable(root.statement_ordinal) {
                        continue;
                    }
                    for value in structural_values
                    .value_dependencies(root.root)
                    .ok_or_else(|| {
                        format!(
                            "structural root {:?} at statement {} has an invalid dependency graph",
                            root.root, root.statement_ordinal
                        )
                    })?
                {
                    if retained_values.contains(&value) {
                        continue;
                    }
                    retained_values.push(value);
                    if let checked_trees::CheckedStructuralValueKind::Record { fields, .. }
                    | checked_trees::CheckedStructuralValueKind::StructuralCase {
                        fields,
                        ..
                    } = &structural_values.nodes.get(value).kind
                    {
                        for field in
                            structural_values
                                .record_fields
                                .span(*fields)
                                .ok_or_else(|| {
                                    format!("constructor {value:?} has no retained field span")
                                })?
                        {
                            let (_, declaration) =
                                program.any_field_owner(field.field).ok_or_else(|| {
                                    format!(
                                        "constructor {value:?} field {:?} has no declaration owner",
                                        field.field
                                    )
                                })?;
                            if declaration.relevance.is_erased()
                                || !matches!(
                                    field.value,
                                    checked_trees::CheckedStructuralRecordFieldValue::Structural(_)
                                )
                            {
                                continue;
                            }
                            // Constructor operands complete in their own storage before
                            // joining the field. Inline field geometry (including byte
                            // views) cannot replace that operand's exact descriptor.
                            retain_operand!(
                                field.type_reference,
                                format_args!("constructor {value:?} field {:?}", field.field)
                            )?;
                        }
                    }
                    if let checked_trees::CheckedStructuralValueKind::Reference {
                        source,
                        source_value,
                        backing_type,
                        ..
                    } = &structural_values.nodes.get(value).kind
                    {
                        // Formation materializes its own permission carrier before a
                        // consumer crosses Referent. A nested producer's descriptor
                        // need not occur in the outer call's signature; its backing
                        // declaration alone cannot supply that stored pointer type.
                        retain_operand!(
                            structural_values.nodes.get(value).type_reference,
                            format_args!("reference formation {value:?} descriptor")
                        )?;
                        retain_declarations(
                        program,
                        shapes,
                        reference_backing_shapes(program, *backing_type, source.type_identity)
                            .ok_or_else(|| {
                                format!(
                                    "reference formation {value:?}: cannot collect backing {} for identity {:?}",
                                    type_description(program, *backing_type),
                                    source.type_identity
                                )
                            })?,
                    )
                    .map_err(|reason| format!("reference formation {value:?} backing: {reason}"))?;
                        if source_value.is_valid() {
                            // A projected computed source also owns its complete
                            // containing storage, independently of the borrowed leaf.
                            retain_operand!(
                                structural_values.nodes.get(*source_value).type_reference,
                                format_args!(
                                    "reference formation {value:?} source {source_value:?}"
                                )
                            )?;
                        }
                    }
                }
                }
                let mut pending: Vec<_> = plans
                    .root_records_for_state(state)
                    .into_iter()
                    .filter(|root| is_executable(root.statement_ordinal))
                    .map(|root| root.root)
                    .collect();
                let mut visited = Vec::new();
                while let Some(handle) = pending.pop() {
                    if !plans.nodes.is_valid(handle) {
                        return Err(format!("scalar computation {handle:?} is not retained"));
                    }
                    if visited.contains(&handle) {
                        continue;
                    }
                    visited.push(handle);
                    match &plans.nodes.get(handle).kind {
                        ComputationKind::CaseMembership {
                            subject:
                                checked_trees::CheckedScalarComputationStructuralArgument::Place(_),
                            receiver_value,
                            ..
                        } if !receiver_value.is_valid() => {}
                        ComputationKind::CaseMembership {
                            subject:
                                checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                            ..
                        } => retain_case_subject_fields(
                            shapes,
                            plans,
                            program,
                            subject,
                            &mut pending,
                        )
                        .map_err(|reason| format!("case membership {handle:?}: {reason}"))?,
                        ComputationKind::SelectedComparison { left, right, .. } => {
                            pending.extend([*left, *right])
                        }
                        checked_trees::cast_or_qualification_triple_pattern!(operand, ..) => {
                            pending.push(*operand)
                        }
                        ComputationKind::ByteSequenceLiteralComparison { operand_type, .. }
                        | ComputationKind::StorageAddress { operand_type, .. }
                        | ComputationKind::ByteSequencePredicate { operand_type, .. } => {
                            retain_operand!(
                                *operand_type,
                                format_args!("scalar computation {handle:?} operand")
                            )?;
                        }
                        ComputationKind::StructuralField { receiver_value, .. }
                        | ComputationKind::CaseMembership { receiver_value, .. } => {
                            if !receiver_value.is_valid() {
                                continue;
                            }
                            if !structural_values.nodes.is_valid(*receiver_value) {
                                return Err(format!(
                                    "scalar computation {handle:?} receiver {receiver_value:?} is not retained"
                                ));
                            }
                            let mut roots =
                                structural_values.roots.iter().filter_map(|(_, root)| {
                                    (root.state == state && root.root == *receiver_value)
                                        .then_some(root)
                                });
                            let root = roots.next().ok_or_else(|| {
                        format!(
                            "scalar computation {handle:?} receiver {receiver_value:?} has no root in this state"
                        )
                    })?;
                            if roots.next().is_some() {
                                return Err(format!(
                                    "scalar computation {handle:?} receiver {receiver_value:?} has multiple roots in this state"
                                ));
                            }
                            if structural_values.nodes.get(*receiver_value).expression
                                != root.expression
                            {
                                return Err(format!(
                                    "scalar computation {handle:?} receiver {receiver_value:?} expression {:?} differs from root expression {:?}",
                                    structural_values.nodes.get(*receiver_value).expression,
                                    root.expression
                                ));
                            }
                            // The evaluated dependency owns the whole receiver type. The
                            // ordinary shape collector retains its transitive fields;
                            // neither the observed leaf nor a source local supplies it.
                            retain_operand!(
                                root.type_reference,
                                format_args!(
                                    "scalar computation {handle:?} receiver {receiver_value:?}"
                                )
                            )?;
                        }
                        ComputationKind::RetainedConstant { .. } | ComputationKind::Value(_) => {}
                        ComputationKind::Dispatch { subject, arms, .. } => {
                            pending.extend(subject.scalar_operands(&plans.operands));
                            if let checked_trees::CheckedDispatchSubject::Structural {
                                value, ..
                            } = subject
                                && value.is_valid()
                            {
                                retain_operand!(
                                    structural_values.nodes.get(*value).type_reference,
                                    format_args!("dispatch {handle:?} subject {value:?}")
                                )?;
                            }
                            for arm in plans.dispatch_arms.span(*arms).ok_or_else(|| {
                                format!("dispatch {handle:?} has no retained arm span")
                            })? {
                                if let Some(value) = arm.pattern.computation() {
                                    pending.push(value);
                                }
                                pending.push(arm.value);
                            }
                        }
                        ComputationKind::Call {
                            callee,
                            arguments,
                            structural_arguments,
                            ..
                        } => {
                            pending.extend(callee.selector());
                            pending.extend(plans.operands.span(*arguments).ok_or_else(|| {
                                format!("call {handle:?} has no retained scalar argument span")
                            })?);
                            for argument in plans
                                .structural_arguments
                                .span(*structural_arguments)
                                .ok_or_else(|| {
                                format!("call {handle:?} has no retained structural argument span")
                            })? {
                                if let Some(place) = argument.as_place() {
                                    pending.extend(place.computation_operands(&plans.operands));
                                    // A borrowed view does not define its scalar ingress's
                                    // storage carrier. Retain the actual authored formal
                                    // so lowering can establish that original home.
                                    if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                                parameter_index,
                            } = place.source
                            {
                                let authored = program.state_by_symbol(state).ok_or_else(|| {
                                    format!("call {handle:?} scalar ingress has no authored state")
                                })?;
                                let parameter = program
                                    .state_parameters(authored)
                                    .iter()
                                    .filter(|parameter| {
                                        !parameter.is_const
                                            && !parameter.relevance.is_erased()
                                            && program
                                                .primitive_type_reference(parameter.type_reference)
                                                .is_some()
                                    })
                                    .nth(parameter_index as usize)
                                    .ok_or_else(|| {
                                        format!(
                                            "call {handle:?} scalar ingress parameter {parameter_index} is absent"
                                        )
                                    })?;
                                retain_operand!(
                                    parameter.type_reference,
                                    format_args!("call {handle:?} scalar ingress parameter {parameter_index}")
                                )?;
                            }
                                }
                                if let checked_trees::CheckedScalarComputationStructuralArgument::Place(
                            argument,
                        ) = argument
                            && let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                expression, value_dependency: Some(value), .. } = argument.source
                        {
                            // Checking retains the actual producer independently
                            // of the home used to read it. A selected copy may
                            // depend on a captured-copy root rather than the
                            // older subject behind its saved expression.
                            let root = structural_values.root_for_value_home(value, expression).ok_or_else(|| {
                                format!("call {handle:?} expression {expression:?} value {value:?} has no completed structural dependency")
                            })?;
                            if root.state != state
                                || !structural_values.nodes.is_valid(value)
                                || structural_values.nodes.get(value).expression != expression
                            {
                                return Err(format!("call {handle:?} expression {expression:?} has a different retained producer {value:?}"));
                            }
                            if argument.path.is_empty() {
                                let producer_identity = validation::structural_type_identity(
                                    program, root.type_reference, &[], &[],
                                );
                                if producer_identity != argument.type_identity {
                                    return Err(format!(
                                        "call {handle:?} expression {expression:?} producer {} differs from argument identity `{}` ({:?})",
                                        type_description(program, root.type_reference),
                                        program.type_identity_text(argument.type_identity).as_deref().unwrap_or("<unretained>"),
                                        argument.type_identity
                                    ));
                                }
                            }
                            if !matches!(argument.access,
                                    checked_trees::CheckedStructuralAccess::Owned
                                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                                        | checked_trees::CheckedStructuralAccess::MutableBorrow
                                        | checked_trees::CheckedStructuralAccess::WriteOnlyBorrow)
                            {
                                return Err(format!(
                                    "call {handle:?} expression {expression:?} has unsupported structural access {:?}",
                                    argument.access
                                ));
                            }
                            // The produced value owns its complete declaration,
                            // including when the invocation only borrows it. The
                            // formal's reference type does not replace this owner;
                            // a projected argument likewise retains its exact path
                            // and transfer without replacing the owner's catalog.
                            // A view root owns descriptor storage, not its
                            // referent: retain its admitted carrier without
                            // reapplying an owned-result admission fence.
                            // its scalar children have their own retained roots in
                            // this same state's computation inventory.
                            retain_operand!(
                                root.type_reference,
                                format_args!("call {handle:?} expression {expression:?} completed producer")
                            )?;
                        }
                                if let checked_trees::CheckedScalarComputationStructuralArgument::Case(
                                subject,
                            ) = argument
                            {
                                retain_case_subject_fields(
                                    shapes,
                                    plans,
                                    program,
                                    subject,
                                    &mut pending,
                                )
                                .map_err(|reason| {
                                    format!("call {handle:?} case argument: {reason}")
                                })?;
                            }
                            }
                        }
                        ComputationKind::Select {
                            condition,
                            when_true,
                            when_false,
                            ..
                        } => pending.extend([*condition, *when_true, *when_false]),
                        ComputationKind::WithOperands {
                            operands, operand, ..
                        } => {
                            pending.push(*operand);
                            for dependency in
                            plans.evaluation_operands.span(*operands).ok_or_else(|| {
                                format!(
                                    "computation {handle:?} has no retained evaluation operand span"
                                )
                            })?
                        {
                            match dependency {
                                checked_trees::CheckedScalarEvaluationOperand::Scalar(value) => {
                                    pending.push(*value)
                                }
                                checked_trees::CheckedScalarEvaluationOperand::Structural(
                                    value,
                                ) => {
                                    let root = structural_values.root_for_value(*value).ok_or_else(|| {
                                    format!("computation {handle:?} structural evaluation operand {value:?} has no retained root")
                                })?;
                                    retain_operand!(
                                        root.type_reference,
                                        format_args!(
                                            "computation {handle:?} structural evaluation operand {value:?}"
                                        )
                                    )?;
                                }
                            }
                        }
                        }
                        ComputationKind::Apply { operands, .. }
                        | ComputationKind::StorageObservation { operands, .. } => {
                            pending.extend(plans.operands.span(*operands).ok_or_else(|| {
                                format!("computation {handle:?} has no retained operand span")
                            })?)
                        }
                    }
                }
                Ok(())
            })()
        }

        fn retain_declarations(
            program: &dyn ::validation::CheckingInput,
            shapes: &mut Vec<TypePlan>,
            declarations: impl IntoIterator<Item = TypePlan>,
        ) -> Result<(), String> {
            for declaration in declarations {
                if let Some(existing) =
                    checked_trees::checked_structural_type_by_identity(shapes, declaration.identity)
                {
                    if existing != &declaration {
                        return Err(format!(
                            "conflicting storage plans for semantic identity `{}` ({:?}): differing fields [{}]",
                            program
                                .type_identity_text(declaration.identity)
                                .as_deref()
                                .unwrap_or("<unretained>"),
                            declaration.identity,
                            differing_plan_fields(existing, &declaration)
                        ));
                    }
                } else {
                    shapes.push(declaration);
                }
            }
            Ok(())
        }

        fn differing_plan_fields(existing: &TypePlan, incoming: &TypePlan) -> String {
            macro_rules! differences {
        ($($field:ident),+ $(,)?) => {
            [$(
                (existing.$field != incoming.$field).then_some(stringify!($field)),
            )+]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ")
        };
    }
            differences!(
                identity,
                native_identity,
                native_semantic_identity,
                native_policy_identity,
                owns_linear_claim,
                copyable,
                no_code_affine_cleanup,
                descriptor_cleanup,
                shape,
                scalar_restriction,
            )
        }

        fn type_description(
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> String {
            if !program
                .type_reference_table
                .contains_type_reference(reference)
            {
                return format!("invalid type reference {reference:?}");
            }
            let identity = validation::structural_type_identity(program, reference, &[], &[]);
            format!(
                "type `{}` (reference {reference:?}, identity {identity:?})",
                program
                    .type_identity_text(identity)
                    .as_deref()
                    .unwrap_or("<unretained>")
            )
        }
    }
    mod guarded_exits {
        /*
            Ordered scalar exits retain every reachable authored guard and selected destination.
            Coverage is separate from evaluation: a final case guard is not a wildcard.
        */
        use super::{CheckedScalarBranchDestination, StatementNode, TransitionGuardNode};
        use crate::execution::terminal_scalar::checked_branch_destination;
        use checked_trees::{CheckedScalarGuardedExit, CheckedScalarGuardedTail};

        pub(super) fn build(
            program: &dyn ::validation::CheckingInput,
            expressions: &checked_trees::CheckedScalarExpressionPlans,
            computations: &checked_trees::CheckedScalarComputationPlans,
        ) -> (
            arena::Arena<CheckedScalarGuardedExit>,
            Vec<CheckedScalarGuardedTail>,
        ) {
            let mut exits = arena::Arena::default();
            let mut tails = Vec::new();
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    if let Some((arms, fallback)) =
                        tail(program, machine, state, expressions, computations)
                    {
                        let mut retained = arena::HandleSpan::empty();
                        for arm in arms {
                            exits.append_to_span(&mut retained, arm);
                        }
                        tails.push(CheckedScalarGuardedTail {
                            state: state.symbol,
                            arms: retained,
                            fallback,
                        });
                    }
                }
            }
            (exits, tails)
        }

        /*
            The tested subject of a `StructuralCaseMembership`/`CaseMembership`
            guard, whichever coordinate retained it. A pure expression names its
            parameter-field path; a computation root names the structural argument
            it observes, which may be a non-parameter subject such as a local.
        */
        #[derive(PartialEq)]
        enum CaseSubject {
            Expression(checked_trees::CheckedStructuralCaseSubject),
            Computation(checked_trees::CheckedScalarComputationStructuralArgument),
        }

        mod tuple_coverage {
            /*
                Finite guard columns share one coverage algorithm across pure and computed operands.
            */
            /*
                One row of a decomposed multi-subject guard: the finite columns an
                authored tuple arm constrains. `Or` unions widen a single column;
                conjuncts outside the column vocabulary leave `residual` — the row
                then proves no cylinder and can only consume coverage other rows
                provide.
            */
            #[derive(Default)]
            pub(super) struct TupleRow {
                columns: Vec<(TupleSubject, Vec<TupleValue>)>,
                residual: bool,
            }

            /*
                The operand one matrix column selects on. A case column is a
                structural path whose active variant the arm requires; a Boolean
                column is a scalar operand the arm pins to a literal.
            */
            #[derive(PartialEq)]
            enum TupleSubject {
                Case(checked_trees::CheckedStructuralCaseSubject),
                ComputedCase {
                    subject: checked_trees::CheckedScalarComputationStructuralArgument,
                    owner: symbols::SymbolHandle,
                },
                Boolean(checked_trees::CheckedBooleanExpression),
            }

            #[derive(Clone, PartialEq)]
            enum TupleValue {
                Case(checked_trees::CheckedMemberIdentity),
                Boolean(bool),
            }

            /*
                Conjunction semantics merge on a shared subject: two columns naming
                the same operand intersect their admitted values.
            */
            fn merge_column(row: &mut TupleRow, subject: TupleSubject, allowed: Vec<TupleValue>) {
                if let Some((_, present)) = row
                    .columns
                    .iter_mut()
                    .find(|(candidate, _)| *candidate == subject)
                {
                    present.retain(|value| allowed.contains(value));
                } else {
                    row.columns.push((subject, allowed));
                }
            }

            /*
                Decompose one `when` guard into its tuple-matrix row. Recognized
                conjuncts are exact case memberships, same-subject `Or` unions of
                them, and `operand == <Boolean literal>` equalities; anything else is
                a residual the coverage pass cannot model.
            */
            pub(super) fn tuple_row(boolean: &checked_trees::CheckedBooleanExpression) -> TupleRow {
                let mut row = TupleRow::default();
                tuple_conjunction(boolean, &mut row);
                row
            }

            fn tuple_conjunction(
                boolean: &checked_trees::CheckedBooleanExpression,
                row: &mut TupleRow,
            ) {
                use checked_trees::CheckedBooleanExpression as Boolean;
                match boolean {
                    Boolean::And { left, right } => {
                        tuple_conjunction(left, row);
                        tuple_conjunction(right, row);
                    }
                    Boolean::StructuralCaseMembership { subject, case } => {
                        merge_column(
                            row,
                            TupleSubject::Case(subject.clone()),
                            vec![TupleValue::Case(case.clone())],
                        );
                    }
                    Boolean::Or { .. } => {
                        let mut pending = vec![boolean];
                        let mut subject: Option<&checked_trees::CheckedStructuralCaseSubject> =
                            None;
                        let mut cases = Vec::new();
                        while let Some(node) = pending.pop() {
                            match node {
                                Boolean::Or { left, right } => {
                                    pending.push(left);
                                    pending.push(right);
                                }
                                Boolean::StructuralCaseMembership {
                                    subject: leaf,
                                    case,
                                } => {
                                    if subject.is_some_and(|prior| *prior != *leaf) {
                                        row.residual = true;
                                        return;
                                    }
                                    subject = Some(leaf);
                                    if !cases.contains(case) {
                                        cases.push(case.clone());
                                    }
                                }
                                _ => {
                                    row.residual = true;
                                    return;
                                }
                            }
                        }
                        let Some(subject) = subject else {
                            row.residual = true;
                            return;
                        };
                        merge_column(
                            row,
                            TupleSubject::Case(subject.clone()),
                            cases.into_iter().map(TupleValue::Case).collect(),
                        );
                    }
                    Boolean::Equal { left, right } => {
                        let (operand, literal) = match (left.as_ref(), right.as_ref()) {
                            (operand, Boolean::Constant(literal))
                            | (Boolean::Constant(literal), operand) => (operand, *literal),
                            _ => {
                                row.residual = true;
                                return;
                            }
                        };
                        merge_column(
                            row,
                            TupleSubject::Boolean(operand.clone()),
                            vec![TupleValue::Boolean(literal)],
                        );
                    }
                    _ => row.residual = true,
                }
            }

            /*
                Whether the decomposed tuple rows provably tile their subjects'
                finite domains. Each case subject's domain enumerates through the
                data owner its path resolves to; a Boolean subject's domain is the
                two literals. A row covers a tuple when every one of its columns
                admits the tuple's value — a subject the row does not constrain is a
                wildcard — and rows carrying a residual prove no slice.
            */
            pub(super) fn tuple_rows_cover(
                program: &dyn ::validation::CheckingInput,
                state: &typed_trees::state::State,
                rows: &[Option<TupleRow>],
            ) -> Option<()> {
                let rows: Vec<&TupleRow> = rows
                    .iter()
                    .map(Option::as_ref)
                    .collect::<Option<Vec<_>>>()?;
                let mut subjects = Vec::new();
                for row in &rows {
                    for (subject, _) in &row.columns {
                        crate::shared::collections::push_unique(&mut subjects, subject);
                    }
                }
                if subjects.is_empty() {
                    return None;
                }
                let parameters = program.state_parameters(state);
                let mut domains = Vec::with_capacity(subjects.len());
                for subject in &subjects {
                    match subject {
                        TupleSubject::Case(field) => {
                            let field = field.parameter_field()?;
                            let (_, _, receiver, _) =
                                crate::values::resolve_structural_parameter_path(
                                    program,
                                    parameters,
                                    field.parameter_position,
                                    &field.path,
                                )?;
                            let definition = crate::values::structural_data(program, receiver)?;
                            let cases: Vec<TupleValue> = program
                                .data_members(definition)
                                .iter()
                                .filter_map(|member| match member {
                                    typed_trees::data::DataMember::Variant(variant) => {
                                        Some(TupleValue::Case(
                                            checked_trees::CheckedMemberIdentity::of_member(
                                                variant.identity,
                                                variant.name.clone(),
                                            ),
                                        ))
                                    }
                                    typed_trees::data::DataMember::Field(_) => None,
                                })
                                .collect::<Vec<_>>();
                            if cases.is_empty() {
                                return None;
                            }
                            domains.push(cases);
                        }
                        TupleSubject::ComputedCase { owner, .. } => {
                            let definition = program.data_definition_by_symbol(*owner)?;
                            let cases = program
                                .data_members(definition)
                                .iter()
                                .filter_map(|member| {
                                    let variant = member.as_variant()?;
                                    Some(TupleValue::Case(
                                        checked_trees::CheckedMemberIdentity::of_member(
                                            variant.identity,
                                            variant.name.clone(),
                                        ),
                                    ))
                                })
                                .collect::<Vec<_>>();
                            if cases.is_empty() {
                                return None;
                            }
                            domains.push(cases);
                        }
                        TupleSubject::Boolean(_) => {
                            domains
                                .push(vec![TupleValue::Boolean(false), TupleValue::Boolean(true)]);
                        }
                    }
                }
                let extent: usize = domains.iter().map(Vec::len).product();
                if extent == 0 || extent > 4096 {
                    return None;
                }
                let mut indexes = vec![0usize; domains.len()];
                loop {
                    let covered = rows.iter().any(|row| {
                        !row.residual
                            && row.columns.iter().all(|(subject, allowed)| {
                                subjects
                                    .iter()
                                    .position(|candidate| *candidate == subject)
                                    .is_some_and(|position| {
                                        allowed.contains(&domains[position][indexes[position]])
                                    })
                            })
                    });
                    if !covered {
                        return None;
                    }
                    let mut axis = domains.len();
                    loop {
                        if axis == 0 {
                            return Some(());
                        }
                        axis -= 1;
                        indexes[axis] += 1;
                        if indexes[axis] < domains[axis].len() {
                            break;
                        }
                        indexes[axis] = 0;
                    }
                }
            }

            /*
                Read Boolean composition in the retained computation's own operand namespace.
                Unsupported operations leave a residual, just as in a pure Boolean row.
            */
            pub(super) fn computation_row(
                program: &dyn validation::CheckingInput,
                plans: &checked_trees::CheckedScalarComputationPlans,
                root: checked_trees::CheckedScalarComputationHandle,
            ) -> TupleRow {
                fn append(target: &mut TupleRow, source: TupleRow) {
                    target.residual |= source.residual;
                    for (subject, values) in source.columns {
                        merge_column(target, subject, values);
                    }
                }
                fn union(mut left: TupleRow, right: TupleRow) -> TupleRow {
                    if !left.residual
                        && !right.residual
                        && let ([(subject, values)], [(other, extra)]) =
                            (left.columns.as_mut_slice(), right.columns.as_slice())
                        && subject == other
                    {
                        for value in extra {
                            if !values.contains(value) {
                                values.push(value.clone());
                            }
                        }
                        return left;
                    }
                    TupleRow {
                        residual: true,
                        ..Default::default()
                    }
                }
                fn constant(
                    plans: &checked_trees::CheckedScalarComputationPlans,
                    root: checked_trees::CheckedScalarComputationHandle,
                ) -> Option<bool> {
                    use checked_trees::{
                        CheckedBooleanExpression as Boolean, CheckedScalarComputationKind as Kind,
                        CheckedScalarExpression as Scalar,
                    };
                    if !plans.nodes.is_valid(root) {
                        return None;
                    }
                    match &plans.nodes.get(root).kind {
                        Kind::Value(Scalar::Boolean(boolean)) => match boolean.as_ref() {
                            Boolean::Constant(value) => Some(*value),
                            _ => None,
                        },
                        _ => None,
                    }
                }
                fn template(
                    program: &dyn validation::CheckingInput,
                    plans: &checked_trees::CheckedScalarComputationPlans,
                    boolean: &checked_trees::CheckedBooleanExpression,
                    operands: &[checked_trees::CheckedScalarComputationHandle],
                    active: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
                ) -> TupleRow {
                    use checked_trees::CheckedBooleanExpression as Boolean;
                    match boolean {
                        Boolean::Parameter { position } => operands
                            .get(*position)
                            .map(|operand| collect(program, plans, *operand, active))
                            .unwrap_or_else(|| TupleRow {
                                residual: true,
                                ..Default::default()
                            }),
                        Boolean::And { left, right } => {
                            let mut row = template(program, plans, left, operands, active);
                            append(&mut row, template(program, plans, right, operands, active));
                            row
                        }
                        Boolean::Or { left, right } => union(
                            template(program, plans, left, operands, active),
                            template(program, plans, right, operands, active),
                        ),
                        _ => TupleRow {
                            residual: true,
                            ..Default::default()
                        },
                    }
                }
                fn collect(
                    program: &dyn validation::CheckingInput,
                    plans: &checked_trees::CheckedScalarComputationPlans,
                    root: checked_trees::CheckedScalarComputationHandle,
                    active: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
                ) -> TupleRow {
                    use checked_trees::{
                        CheckedScalarComputationKind as Kind, CheckedScalarExpression as Scalar,
                    };
                    if !plans.nodes.is_valid(root) || active.contains(&root) {
                        return TupleRow {
                            residual: true,
                            ..Default::default()
                        };
                    }
                    active.push(root);
                    let row = match &plans.nodes.get(root).kind {
                        Kind::CaseMembership { subject, case, .. } => {
                            let owner = program.symbols.get(*case).parent;
                            let variant =
                                program.data_definition_by_symbol(owner).and_then(|data| {
                                    program
                                        .data_members(data)
                                        .iter()
                                        .filter_map(|member| member.as_variant())
                                        .find(|variant| variant.symbol == *case)
                                });
                            if let Some(variant) = variant {
                                TupleRow {
                                    columns: vec![(
                                        TupleSubject::ComputedCase {
                                            subject: subject.clone(),
                                            owner,
                                        },
                                        vec![TupleValue::Case(
                                            checked_trees::CheckedMemberIdentity::of_member(
                                                variant.identity,
                                                variant.name.clone(),
                                            ),
                                        )],
                                    )],
                                    residual: false,
                                }
                            } else {
                                TupleRow {
                                    residual: true,
                                    ..Default::default()
                                }
                            }
                        }
                        Kind::Select {
                            condition,
                            when_true,
                            when_false,
                            ..
                        } if constant(plans, *when_false) == Some(false) => {
                            let mut row = collect(program, plans, *condition, active);
                            append(&mut row, collect(program, plans, *when_true, active));
                            row
                        }
                        Kind::Select {
                            condition,
                            when_true,
                            when_false,
                            ..
                        } if constant(plans, *when_true) == Some(true) => union(
                            collect(program, plans, *condition, active),
                            collect(program, plans, *when_false, active),
                        ),
                        Kind::Value(Scalar::Boolean(boolean)) => tuple_row(boolean),
                        Kind::Apply {
                            expression: Scalar::Boolean(boolean),
                            operands,
                            ..
                        } => {
                            if let Some(operands) = plans.operands.span(*operands) {
                                template(program, plans, boolean, operands, active)
                            } else {
                                TupleRow {
                                    residual: true,
                                    ..Default::default()
                                }
                            }
                        }
                        _ => TupleRow {
                            residual: true,
                            ..Default::default()
                        },
                    };
                    active.pop();
                    row
                }
                collect(program, plans, root, &mut Vec::new())
            }
        }
        use tuple_coverage::{computation_row, tuple_row, tuple_rows_cover};

        fn tail(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            expressions: &checked_trees::CheckedScalarExpressionPlans,
            computations: &checked_trees::CheckedScalarComputationPlans,
        ) -> Option<(
            Vec<CheckedScalarGuardedExit>,
            Option<CheckedScalarBranchDestination>,
        )> {
            let statements = program.statement_table.statements(state.statement_nodes);
            let prefix = statements
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
            let mut arms = Vec::new();
            let mut fallback = None;
            let mut case_subject = None;
            let mut cases = Vec::new();
            let mut exact_cases = true;
            let mut rows = Vec::new();
            let tail = typed_trees::statement::reachable_transition_prefix(&statements[prefix..]);
            for (offset, statement) in tail.iter().enumerate() {
                let ordinal = u32::try_from(prefix.checked_add(offset)?).ok()?;
                if fallback.is_some() {
                    return None;
                }
                let Some(transition) = statement.as_transition() else {
                    if let StatementNode::Expression(expression) = statement
                        && !arms.is_empty()
                    {
                        fallback = Some(
                            if validation::expression_normal_completion_possible(
                                program,
                                *expression,
                            ) == Some(false)
                            {
                                CheckedScalarBranchDestination::InvokeNoNormal {
                                    statement_ordinal: ordinal,
                                    is_continuation: false,
                                }
                            } else {
                                CheckedScalarBranchDestination::Return {
                                    statement_ordinal: ordinal,
                                    is_continuation: false,
                                }
                            },
                        );
                        continue;
                    }
                    return None;
                };
                if transition.continuation.is_valid() {
                    return None;
                }
                let destination =
                    checked_branch_destination(program, machine, ordinal, transition, false)?;
                match transition.guard {
                    TransitionGuardNode::Always if !arms.is_empty() => fallback = Some(destination),
                    TransitionGuardNode::When(guard) => {
                        let role = checked_trees::CheckedScalarExpressionRole::Guard;
                        let mut selected = expressions.expressions.iter().filter(|expression| {
                            expression.state == state.symbol
                                && expression.statement_ordinal == ordinal
                                && expression.role == role
                        });
                        let row;
                        let case_evidence = if let Some(expression) = selected.next() {
                            if selected.next().is_some() {
                                return None;
                            }
                            let checked_trees::CheckedScalarExpression::Boolean(boolean) =
                                &expression.expression
                            else {
                                return None;
                            };
                            row = Some(tuple_row(boolean.as_ref()));
                            if let checked_trees::CheckedBooleanExpression::StructuralCaseMembership {
                                subject,
                                ..
                            } = boolean.as_ref()
                            {
                                let (_, case) =
                                    crate::published::proof::exact_outcome_case_test(program, guard)?;
                                Some((CaseSubject::Expression(subject.clone()), case))
                            } else {
                                None
                            }
                        } else {
                            let root = computations
                                .root_at(state.symbol, ordinal, role)
                                .filter(|root| root.machine == machine.symbol)?;
                            row = Some(computation_row(program, computations, root.root));
                            let node = computations.nodes.get(root.root);
                            if node.primitive_type != checked_trees::types::PrimitiveType::Bool {
                                return None;
                            }
                            match &node.kind {
                                checked_trees::CheckedScalarComputationKind::CaseMembership {
                                    subject,
                                    case,
                                    ..
                                } => Some((CaseSubject::Computation(subject.clone()), *case)),
                                _ => None,
                            }
                        };
                        match case_evidence {
                            Some((subject, case)) => {
                                if case_subject.as_ref().is_some_and(|prior| *prior != subject) {
                                    exact_cases = false;
                                }
                                case_subject = Some(subject);
                                if cases.contains(&case) {
                                    exact_cases = false;
                                }
                                cases.push(case);
                            }
                            None => exact_cases = false,
                        }
                        rows.push(row);
                        arms.push(CheckedScalarGuardedExit {
                            guard_statement_ordinal: ordinal,
                            destination,
                        });
                    }
                    _ => return None,
                }
            }
            if arms.is_empty() {
                return None;
            }
            let complementary_pair = arms.len() == 2
                && fallback.is_none()
                && crate::execution::guard_complement::complementary(
                    program,
                    expressions,
                    state,
                    u32::try_from(prefix).ok()?,
                );
            if fallback.is_none() && !complementary_pair {
                if exact_cases {
                    let first = *cases.first()?;
                    let owner = program.symbols.get(first).parent;
                    let data = program.data_definition_by_symbol(owner)?;
                    let expected = program
                        .data_members(data)
                        .iter()
                        .filter_map(|member| match member {
                            typed_trees::data::DataMember::Variant(variant) => Some(variant.symbol),
                            typed_trees::data::DataMember::Field(_) => None,
                        })
                        .collect::<Vec<_>>();
                    if expected.len() != cases.len()
                        || expected.iter().any(|case| !cases.contains(case))
                    {
                        return None;
                    }
                } else {
                    tuple_rows_cover(program, state, &rows)?;
                }
            }
            Some((arms, fallback))
        }
    }

    /* Every state's ordered guarded exits and fallback, for every machine. */
    pub(crate) fn build_checked_guarded_exit_plans(
        program: &dyn ::validation::CheckingInput,
        expressions: &checked_trees::CheckedScalarExpressionPlans,
        computations: &checked_trees::CheckedScalarComputationPlans,
    ) -> checked_trees::CheckedGuardedExitPlans {
        let (exits, tails) = guarded_exits::build(program, expressions, computations);
        checked_trees::CheckedGuardedExitPlans { exits, tails }
    }

    fn checked_successor(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        statement_ordinal: u32,
        transition: &typed_trees::statement::TableTransition,
        is_continuation: bool,
    ) -> Option<CheckedStructuralControlSuccessorPlan> {
        if transition.exit != TransitionExit::Ordinary {
            return None;
        }
        let TransitionTargetNode::Named { path, .. } =
            program
                .statement_table
                .transition_target(if is_continuation {
                    transition.continuation
                } else {
                    transition.target
                })
        else {
            return None;
        };
        let target = if let Some(target_index) =
            validation::activation_target_state_index(program, machine, path.symbol)
        {
            program.machine_states(machine).get(target_index)?.symbol
        } else {
            validation::find_machine_head(program, path.symbol)?
                .1
                .symbol
        };
        Some(CheckedStructuralControlSuccessorPlan::new(
            statement_ordinal,
            target,
            classify(program, machine, path.symbol),
            is_continuation,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ))
    }

    /* Retain authored exit coordinates independently of the preceding operation values. */
    pub(super) fn checked_terminator(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expressions: &checked_trees::CheckedScalarExpressionPlans,
        binding_count: usize,
    ) -> Option<CheckedScalarStateTerminator> {
        let statements = program.statement_table.statements(state.statement_nodes);
        let terminator_ordinal = u32::try_from(binding_count).ok()?;
        if let Some(selected) = selected_transition(program, statements, binding_count) {
            let ordinal = selected.statement_ordinal;
            let transition = selected.transition;
            return match checked_branch_destination(program, machine, ordinal, transition, false)? {
                CheckedScalarBranchDestination::Return {
                    statement_ordinal,
                    is_continuation: false,
                } => Some(CheckedScalarStateTerminator::Return { statement_ordinal }),
                CheckedScalarBranchDestination::Crash { statement_ordinal } => {
                    Some(CheckedScalarStateTerminator::Crash { statement_ordinal })
                }
                CheckedScalarBranchDestination::Jump(successor) => {
                    Some(CheckedScalarStateTerminator::Jump(successor))
                }
                _ => None,
            };
        }
        let reachable =
            typed_trees::statement::reachable_transition_prefix(&statements[binding_count..]);
        Some(match reachable {
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
                rest @ ..,
            ] if matches!(when_true.guard, TransitionGuardNode::When(_))
                && (when_false.guard == TransitionGuardNode::Always
                    || complementary(program, expressions, state, terminator_ordinal))
                && !when_true.continuation.is_valid()
                && !when_false.continuation.is_valid()
                && rest.iter().all(|statement| {
                    matches!(
                        statement,
                        StatementNode::Transition(transition)
                            if transition.guard == TransitionGuardNode::Always
                                && transition.exit == TransitionExit::Ordinary
                                && !transition.continuation.is_valid()
                    )
                }) =>
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
        program: &dyn ::validation::CheckingInput,
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
            TransitionTargetNode::Terminal
                if !is_continuation
                    && transition.exit == TransitionExit::Ordinary
                    && !transition.continuation.is_valid() =>
            {
                Some(CheckedScalarBranchDestination::Return {
                    statement_ordinal,
                    is_continuation: false,
                })
            }
            TransitionTargetNode::Value(expression)
                if program.expression_table.expression_is_valid(*expression) =>
            {
                Some(
                    if validation::expression_normal_completion_possible(program, *expression)
                        == Some(false)
                    {
                        CheckedScalarBranchDestination::InvokeNoNormal {
                            statement_ordinal,
                            is_continuation,
                        }
                    } else {
                        CheckedScalarBranchDestination::Return {
                            statement_ordinal,
                            is_continuation,
                        }
                    },
                )
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
}
#[path = "execution_terminal_unit.rs"]
pub(crate) mod terminal_unit;

/*
    Every recorded ownership permission event, in arena order.

    The arena is the one home those events have; each consumer filters by
    machine, state, source and kind at its own site.
*/
pub(crate) fn permission_events(
    facts: &CheckFacts,
) -> impl Iterator<Item = (Handle<FlowPermissionEventFact>, &FlowPermissionEventFact)> {
    facts.flow.ownership.permissions.iter()
}
