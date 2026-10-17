//! Terminal dispositions reuse the ordinary claim ledger. A consuming boundary
//! requirement must be authored by the resource owner, the proposition must
//! name one exact live owned argument, and an unspent modeled instruction must
//! establish it at this point. Logical truth alone grants no terminal disposal.
use super::linear_obligations::LinearPlace;
use checked_trees::{CheckFacts, FlowPermissionEvaluation, FlowPermissionEventFact};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::statement::{AssemblyFactKind, StatementNode};
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone, Default)]
pub(super) struct InstructionDispositions {
    spent: Vec<crate::checks::contracts::prover::InstructionEffectEvidence>,
}
impl InstructionDispositions {
    pub(super) fn statement(
        &mut self,
        program: &dyn validation::CheckingInput,
        facts: &mut CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement: usize,
        evaluation: FlowPermissionEvaluation,
        places: &mut [LinearPlace],
        events: &mut Vec<FlowPermissionEventFact>,
    ) {
        if evaluation != FlowPermissionEvaluation::Statement {
            return;
        }
        let Some(StatementNode::AssemblyFact(assertion)) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement)
        else {
            return;
        };
        if assertion.kind != AssemblyFactKind::Ensures {
            return;
        }
        let Some(flow) = facts
            .flow
            .control
            .states
            .iter()
            .find(|(_, flow)| flow.state_symbol == state.symbol)
            .map(|(_, flow)| flow)
        else {
            return;
        };
        let Some(row) = facts.flow.state_statement(flow, statement) else {
            return;
        };
        let contexts = facts
            .flow
            .semantic_constraint_contexts(row.entry_constraints)
            .collect::<Vec<_>>();
        self.settle(
            program,
            facts,
            machine,
            state,
            statement,
            PermissionEventSource::InstructionDisposition {
                statement_index: statement,
            },
            evaluation,
            assertion.fact,
            &contexts,
            places,
            events,
        );
    }
    pub(super) fn exit(
        &mut self,
        program: &dyn validation::CheckingInput,
        facts: &mut CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        coordinate: checked_trees::FlowClaimJoinExit,
        places: &mut [LinearPlace],
        events: &mut Vec<FlowPermissionEventFact>,
    ) {
        let evaluation = super::linear_validation::exit_evaluation(program, state, coordinate);
        let target = match evaluation {
            FlowPermissionEvaluation::TransitionTarget(target) => target,
            _ => Default::default(),
        };
        let exits = facts
            .flow
            .control
            .exits
            .iter()
            .filter_map(|(_, exit)| {
                (exit.machine_symbol == machine.symbol
                    && exit.state_symbol == state.symbol
                    && exit.statement_index == coordinate.statement_index
                    && exit.transition_target == target)
                    .then_some(exit.clone())
            })
            .collect::<Vec<_>>();
        for exit in exits {
            let contexts = facts
                .flow
                .semantic_constraint_contexts(exit.entry_constraints)
                .collect::<Vec<_>>();
            let mut contracts = Vec::new();
            for context in facts
                .flow
                .semantic_constraint_contexts(exit.ensures_constraints)
            {
                for fact in facts
                    .semantic
                    .context_view(facts.semantic.contexts.get(context))
                    .facts()
                {
                    let source = match fact.payload {
                        facts::FactPayload::PropositionApplication { fact, .. }
                        | facts::FactPayload::ContractPropositionApplication { fact, .. } => fact,
                        _ => continue,
                    };
                    if !contracts.contains(&source) {
                        contracts.push(source);
                    }
                }
            }
            for contract in contracts {
                self.settle(
                    program,
                    facts,
                    machine,
                    state,
                    coordinate.statement_index,
                    PermissionEventSource::InstructionExitDisposition {
                        statement_index: coordinate.statement_index,
                    },
                    evaluation,
                    contract,
                    &contexts,
                    places,
                    events,
                );
            }
        }
    }
    fn settle(
        &mut self,
        program: &dyn validation::CheckingInput,
        facts: &mut CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement: usize,
        source: PermissionEventSource,
        evaluation: FlowPermissionEvaluation,
        fact: arena::Handle<ProofFact>,
        contexts: &[facts::FactContextHandle],
        places: &mut [LinearPlace],
        events: &mut Vec<FlowPermissionEventFact>,
    ) {
        let Some(application) = program.proof_facts.get(fact).as_proposition() else {
            return;
        };
        if validation::instruction_effect_contract(program, application).is_none() {
            return;
        }
        let mut candidates = Vec::new();
        for argument in program
            .expression_table
            .expression_handles(application.arguments)
        {
            let Some(place) = validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement,
                *argument,
            ) else {
                continue;
            };
            let Some(position) = places.iter().position(|held| {
                held.live
                    && held.ever_established
                    && held.multiplicity == Multiplicity::Linear
                    && held.root == place.root
                    && held.path == place.segments
                    && held
                        .claim_identity
                        .is_some_and(|identity| identity != PermissionClaimIdentity::Unknown)
                    && held
                        .provenance
                        .is_some_and(|origin| origin != PermissionProvenance::Unknown)
            }) else {
                continue;
            };
            if candidates.iter().all(|(other, _)| *other != position) {
                candidates.push((position, *argument));
            }
        }
        let [(position, argument)] = candidates.as_slice() else {
            return;
        };
        let position = *position;
        let argument = *argument;
        let Some(reference) = validation::canonical_place_type_reference(
            program,
            state.symbol,
            statement,
            &validation::CanonicalPlace {
                root: places[position].root,
                segments: places[position].path.clone(),
            },
        ) else {
            return;
        };
        let Some(requirement) = consuming_requirement(program, machine, reference) else {
            return;
        };
        let Some(mut event) = crate::checks::contracts::prover::instruction_effect_evidence(
            program,
            facts,
            state.symbol,
            statement,
            contexts,
            application,
            &self.spent,
        ) else {
            return;
        };
        let held = &mut places[position];
        let segments = facts
            .flow
            .ownership
            .segments
            .insert_many(held.path.iter().copied());
        if matches!(
            source,
            PermissionEventSource::InstructionExitDisposition { .. }
        ) {
            event.observe_at(facts::ProgramPoint::Exit {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index: statement,
                transition_target: match evaluation {
                    FlowPermissionEvaluation::TransitionTarget(target) => target,
                    _ => Default::default(),
                },
            });
        }
        let evidence = event.publish(&mut facts.flow.ownership.instruction_effect_receipts);
        facts.flow.ownership.instruction_dispositions.append(
            checked_trees::flow::FlowInstructionDispositionFact {
                machine: machine.symbol,
                state: state.symbol,
                source,
                evaluation,
                requirement,
                fact,
                argument,
                event: event.event,
                evidence,
                root: held.root,
                segments,
            },
        );
        events.push(FlowPermissionEventFact {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            source,
            evaluation,
            expression: Default::default(),
            kind: PermissionEventKind::Consume,
            multiplicity: held.multiplicity,
            access: PermissionAccess::Owned,
            claim_identity: held
                .claim_identity
                .expect("a selected live claim has identity"),
            provenance: held
                .provenance
                .expect("a selected live claim has provenance"),
            root: held.root,
            segments,
            obligation_live: true,
        });
        held.live = false;
        self.spent.push(event);
    }
}

fn consuming_requirement(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    if super::type_reference_is_reference(program, reference) {
        return None;
    }
    let reference = validation::peel_declared_constraints(program, reference);
    if !matches!(
        program.type_reference_table.type_reference(reference),
        typed_trees::types::TypeReferenceNode::Named { .. }
            | typed_trees::types::TypeReferenceNode::Generic { .. }
    ) {
        return None;
    }
    let resource = program.data_definition_by_symbol(program.type_reference_symbol(reference))?;
    let resource_identity = program.package_qualified_type_identity(reference);
    let entry = program.machine_states(machine).first()?;
    let actual = program.state_parameters(entry);
    let mut declarations = Vec::new();
    for conformance in program.machine_trait_conformances(machine) {
        if let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
            definition,
            requirement,
        }) = typed_trees::machine::resolve_satisfied_declaration(program, machine, conformance)
        {
            declarations.push((definition, requirement));
        }
    }
    for conformance in program.conformances() {
        for row in &conformance.rows {
            if row.realization_machine != machine.symbol {
                continue;
            }
            if let Some(definition) = program.trait_definition_by_symbol(row.declaring_trait)
                && let Some(requirement) =
                    program.trait_machine_signature_by_symbol(definition, row.requirement)
            {
                declarations.push((definition, requirement));
            }
        }
    }
    declarations
        .into_iter()
        .find_map(|(definition, requirement)| {
            if !definition.is_boundary
                || !program
                    .symbols
                    .same_symbol_source_package(definition.symbol, resource.symbol)
            {
                return None;
            }
            let formal = program.state_signature_parameters(requirement);
            if formal.len() != actual.len() {
                return None;
            }
            formal
                .iter()
                .zip(actual)
                .any(|(formal, actual)| {
                    !super::type_reference_is_reference(program, formal.type_reference)
                        && !super::type_reference_is_reference(program, actual.type_reference)
                        && super::linear_validation::linear_claim_frontier(
                            program,
                            actual.type_reference,
                        )
                        .iter()
                        .any(|claim| {
                            program.package_qualified_type_identity(
                                validation::peel_declared_constraints(
                                    program,
                                    claim.type_reference,
                                ),
                            ) == resource_identity
                        })
                })
                .then_some(requirement.symbol)
        })
}

/// Bind the published permission occurrence to the retained proof receipt.
/// Proof search happens once in production; replay verifies custody identities
/// and this source association instead of reconstructing the instruction CFG.
pub(super) fn valid_receipt(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    event: &FlowPermissionEventFact,
) -> bool {
    let statement_index = match event.source {
        PermissionEventSource::InstructionDisposition { statement_index }
        | PermissionEventSource::InstructionExitDisposition { statement_index } => statement_index,
        _ => return false,
    };
    let mut receipts = facts
        .flow
        .ownership
        .instruction_dispositions
        .iter()
        .filter_map(|(_, receipt)| {
            (receipt.machine == machine.symbol
                && receipt.state == state.symbol
                && receipt.source == event.source
                && receipt.evaluation == event.evaluation
                && receipt.root == event.root
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(receipt.segments)
                    == facts.flow.ownership.segments.span_or_empty(event.segments))
            .then_some(receipt)
        });
    let Some(receipt) = receipts.next() else {
        return false;
    };
    let observation = match event.source {
        PermissionEventSource::InstructionDisposition { .. } => facts::ProgramPoint::Statement {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index,
        },
        PermissionEventSource::InstructionExitDisposition { .. } => facts::ProgramPoint::Exit {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index,
            transition_target: match event.evaluation {
                FlowPermissionEvaluation::TransitionTarget(target) => target,
                _ => Default::default(),
            },
        },
        _ => return false,
    };
    if !facts
        .flow
        .ownership
        .instruction_effect_receipts
        .is_valid(receipt.evidence)
        || facts
            .flow
            .ownership
            .instruction_effect_receipts
            .get(receipt.evidence)
            .observation
            != observation
    {
        return false;
    }
    if receipts.next().is_some()
        || event.kind != PermissionEventKind::Consume
        || event.access != PermissionAccess::Owned
        || !event.obligation_live
        || !facts.semantic.facts.is_valid(receipt.event)
    {
        return false;
    }
    match event.source {
        PermissionEventSource::InstructionDisposition { .. } => {
            let Some(StatementNode::AssemblyFact(assertion)) = program.statement_table.statements(state.statement_nodes).get(statement_index)
            else { return false; };
            if assertion.kind != AssemblyFactKind::Ensures || assertion.fact != receipt.fact { return false; }
        }
        PermissionEventSource::InstructionExitDisposition { .. } => {
            if !facts.flow.control.exits.iter().any(|(_, exit)| {
                exit.machine_symbol == machine.symbol && exit.state_symbol == state.symbol
                    && exit.statement_index == statement_index
                    && facts.flow.semantic_constraint_contexts(exit.ensures_constraints).any(|context| {
                        facts.semantic.context_view(facts.semantic.contexts.get(context)).facts().any(|fact| {
                            matches!(fact.payload, facts::FactPayload::PropositionApplication { fact, .. }
                                | facts::FactPayload::ContractPropositionApplication { fact, .. } if fact == receipt.fact)
                        })
                    })
            }) { return false; }
        }
        _ => return false,
    }
    let Some(application) = program.proof_facts.get(receipt.fact).as_proposition() else {
        return false;
    };
    let Some(contract) = validation::instruction_effect_contract(program, application) else {
        return false;
    };
    if !valid_effect_tree(
        program,
        facts,
        receipt.evidence,
        receipt.event,
        contract.operation,
    ) || !program
        .expression_table
        .expression_handles(application.arguments)
        .contains(&receipt.argument)
    {
        return false;
    }
    let Some(place) = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        receipt.argument,
    ) else {
        return false;
    };
    place.root == receipt.root
        && place.segments
            == facts
                .flow
                .ownership
                .segments
                .span_or_empty(receipt.segments)
        && validation::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &place,
        )
        .and_then(|reference| consuming_requirement(program, machine, reference))
            == Some(receipt.requirement)
}

fn valid_effect_tree(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    handle: arena::Handle<checked_trees::flow::FlowInstructionEffectReceiptFact>,
    event: facts::FactHandle,
    operation: facts::InstructionEffectOperation,
) -> bool {
    let arena = &facts.flow.ownership.instruction_effect_receipts;
    if !arena.is_valid(handle) {
        return false;
    }
    let root = arena.get(handle);
    if root.event != event {
        return false;
    }
    let mut pending = vec![handle];
    while let Some(handle) = pending.pop() {
        let receipt = arena.get(handle);
        if !facts.semantic.facts.is_valid(receipt.event) {
            return false;
        }
        let effect = facts.semantic.facts.get(receipt.event);
        if !effect_visible_at(facts, receipt.observation, receipt.event) {
            return false;
        }
        if operation == facts::InstructionEffectOperation::AtomicReleaseStore
            && matches!(
                effect.payload,
                facts::FactPayload::InstructionEffect {
                    operation: facts::InstructionEffectOperation::AtomicReleaseStore
                }
            )
        {
            let facts::ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            } = effect.point
            else {
                return false;
            };
            let Some(machine) = program.machine_by_symbol(machine_symbol) else {
                return false;
            };
            let Some(state) = program.state_by_symbol(state_symbol) else {
                return false;
            };
            let Some(statement) = program
                .statement_table
                .statements(state.statement_nodes)
                .get(statement_index)
            else {
                return false;
            };
            if crate::flow::atomic_effects::release_store(program, machine, state, statement)
                .is_none()
                || !arena
                    .span(receipt.children)
                    .is_some_and(|children| children.is_empty())
            {
                return false;
            }
            continue;
        }
        let facts::ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } = effect.point
        else {
            return false;
        };
        let Some(flow) = facts.flow.control.state(machine_symbol, state_symbol) else {
            return false;
        };
        let Some(call) = facts
            .flow
            .control
            .calls
            .span_or_empty(flow.calls)
            .iter()
            .find(|call| {
                call.statement_index == statement_index && call.call_ordinal == call_ordinal
            })
        else {
            return false;
        };
        let Some(children) = arena.span(receipt.children) else {
            return false;
        };
        if children.is_empty() {
            if !matches!(effect.payload, facts::FactPayload::InstructionEffect { operation: recorded } if recorded == operation)
                || program
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    .and_then(|builtin| {
                        language_core::inline_assembly::AsmRegisterOperation::from_intrinsic_name(
                            builtin.name(),
                        )
                    })
                    .map(facts::InstructionEffectOperation::Register)
                    != Some(operation)
            {
                return false;
            }
        } else {
            if !matches!(effect.payload, facts::FactPayload::CallCompletion { target } if target == call.target_symbol)
                || !facts.proof.contract_facts.is_valid(receipt.contract)
            {
                return false;
            }
            let guarantee = facts.proof.contract_facts.get(receipt.contract);
            if guarantee.kind != checked_trees::ContractProofFactKind::Ensures
                || !facts
                    .proof
                    .contract_fact_refs
                    .span_or_empty(call.ensures)
                    .iter()
                    .any(|reference| reference.fact == receipt.contract)
            {
                return false;
            }
            let Some(application) = program.proof_facts.get(guarantee.fact).as_proposition() else {
                return false;
            };
            if validation::instruction_effect_contract(program, application)
                .is_none_or(|contract| contract.operation != operation)
            {
                return false;
            }
            let Some(callee) = facts
                .flow
                .control
                .states
                .iter()
                .map(|(_, state)| state)
                .find(|state| state.state_symbol == call.target_symbol)
            else {
                return false;
            };
            let exits = facts
                .flow
                .control
                .exits
                .iter()
                .filter_map(|(_, exit)| {
                    (exit.machine_symbol == callee.machine_symbol).then_some(exit)
                })
                .collect::<Vec<_>>();
            if exits.len() != children.len()
                || !exits.iter().all(|exit| {
                    let observation = facts::ProgramPoint::Exit {
                        machine_symbol: exit.machine_symbol,
                        state_symbol: exit.state_symbol,
                        statement_index: exit.statement_index,
                        transition_target: exit.transition_target,
                    };
                    children
                        .iter()
                        .filter(|child| child.observation == observation)
                        .count()
                        == 1
                })
            {
                return false;
            }
            // Arena edges always point backwards; a malformed cyclic receipt
            // cannot turn a preserved premise into a fresh instruction.
            for child in receipt.children.handles() {
                if child.arena_index() >= handle.arena_index() {
                    return false;
                }
                pending.push(child);
            }
        }
    }
    true
}

/// Visibility is bound to the producer's actual statement or selected exit.
/// Replaying a checked receipt does not repeat scalar proof search.
fn effect_visible_at(
    facts: &CheckFacts,
    point: facts::ProgramPoint,
    event: facts::FactHandle,
) -> bool {
    let constraints = match point {
        facts::ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index,
        } => {
            let Some(state) = facts.flow.control.state(machine_symbol, state_symbol) else {
                return false;
            };
            let Some(statement) = facts.flow.state_statement(state, statement_index) else {
                return false;
            };
            statement.entry_constraints
        }
        facts::ProgramPoint::Exit {
            machine_symbol,
            state_symbol,
            statement_index,
            transition_target,
        } => {
            let Some(exit) = facts
                .flow
                .control
                .exits
                .iter()
                .map(|(_, exit)| exit)
                .find(|exit| {
                    exit.machine_symbol == machine_symbol
                        && exit.state_symbol == state_symbol
                        && exit.statement_index == statement_index
                        && exit.transition_target == transition_target
                })
            else {
                return false;
            };
            exit.entry_constraints
        }
        _ => return false,
    };
    facts
        .flow
        .semantic_constraint_contexts(constraints)
        .any(|context| {
            facts
                .semantic
                .refs
                .span_or_empty(facts.semantic.contexts.get(context).facts)
                .iter()
                .any(|reference| reference.fact == event)
        })
}
