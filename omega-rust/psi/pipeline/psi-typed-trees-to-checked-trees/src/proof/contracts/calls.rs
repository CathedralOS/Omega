use super::*;
use crate::lookup::machine_state_count;

pub(crate) fn build_contract_call_facts(
    program: &psi_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    contract_facts: &psi_arena::Arena<ContractProofFact>,
) -> (
    psi_arena::Arena<ContractProofFactRef>,
    psi_arena::Arena<ContractCallFact>,
) {
    let mut fact_refs = psi_arena::Arena::with_capacity(contract_facts.len());
    let mut calls = psi_arena::Arena::with_capacity(borrow.calls.len());

    for state in borrow.states.iter().map(|(_, state)| state) {
        for call in borrow.calls.span_or_empty(state.calls) {
            let Some((target_machine_symbol, target_state_symbol)) =
                contract_target_from_state_symbol(program, call.target_symbol)
            else {
                continue;
            };

            let call_site = crate::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            );
            let contract_target = call_site
                .as_ref()
                .and_then(crate::CallSite::static_requirement_dispatch)
                .map(|dispatch| (dispatch.declaring_trait, dispatch.requirement))
                .unwrap_or((target_machine_symbol, target_state_symbol));
            append_contract_call(
                contract_facts,
                &mut fact_refs,
                &mut calls,
                call_site
                    .as_ref()
                    .is_some_and(|site| !crate::call_site_evidence_arguments(site).is_empty()),
                ContractCallSite {
                    caller_machine_symbol: state.machine_symbol,
                    caller_state_symbol: state.state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target_machine_symbol,
                    target_state_symbol,
                    contract_machine_symbol: contract_target.0,
                    contract_state_symbol: contract_target.1,
                    is_state_transfer: matches!(
                        call_site,
                        Some(crate::CallSite::TransitionNamed { .. })
                    ),
                },
            );
        }
    }

    (fact_refs, calls)
}

#[derive(Debug, Clone, Copy)]
struct ContractCallSite {
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    target_machine_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
    contract_machine_symbol: SymbolHandle,
    contract_state_symbol: SymbolHandle,
    is_state_transfer: bool,
}

fn append_contract_call(
    contract_facts: &psi_arena::Arena<ContractProofFact>,
    fact_refs: &mut psi_arena::Arena<ContractProofFactRef>,
    calls: &mut psi_arena::Arena<ContractCallFact>,
    has_authored_evidence_arguments: bool,
    site: ContractCallSite,
) {
    let requires = append_contract_fact_refs(
        contract_facts,
        fact_refs,
        site.contract_machine_symbol,
        Some(site.contract_state_symbol),
        ContractProofFactKind::Requires,
        !site.is_state_transfer,
    );
    // A jump does not return to its source or establish the machine's
    // postconditions. Those remain obligations at the eventual normal exit.
    let ensures = if site.is_state_transfer {
        HandleSpan::empty()
    } else {
        append_contract_fact_refs(
            contract_facts,
            fact_refs,
            site.contract_machine_symbol,
            Some(site.contract_state_symbol),
            ContractProofFactKind::Ensures,
            true,
        )
    };

    if requires.is_empty() && ensures.is_empty() && !has_authored_evidence_arguments {
        return;
    }

    calls.append(ContractCallFact {
        caller_machine_symbol: site.caller_machine_symbol,
        caller_state_symbol: site.caller_state_symbol,
        statement_index: site.statement_index,
        call_ordinal: site.call_ordinal,
        target_machine_symbol: site.target_machine_symbol,
        target_state_symbol: site.target_state_symbol,
        requires,
        ensures,
        evidence_arguments: HandleSpan::empty(),
    });
}

pub(crate) fn build_contract_exit_facts(
    program: &psi_typed_trees::TypedTrees,
    contract_facts: &psi_arena::Arena<ContractProofFact>,
    fact_refs: &mut psi_arena::Arena<ContractProofFactRef>,
) -> psi_arena::Arena<ContractExitFact> {
    let mut exits = psi_arena::Arena::with_capacity(machine_state_count(program));

    for machine in program.machines() {
        if !machine.body_is_present {
            continue;
        }
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            let ensures = append_contract_fact_refs(
                contract_facts,
                fact_refs,
                machine.symbol,
                Some(state.symbol),
                ContractProofFactKind::Ensures,
                true,
            );

            if ensures.is_empty() {
                continue;
            }

            let mut has_transition = false;
            for (statement_index, statement) in statements.iter().enumerate() {
                let StatementNode::Transition(transition) = statement else {
                    continue;
                };
                has_transition = true;
                if transition.exit == psi_typed_trees::statement::TransitionExit::Ordinary {
                    for target in [transition.target, transition.continuation] {
                        if target.is_valid()
                            && matches!(
                                program.statement_table.transition_target(target),
                                psi_typed_trees::statement::TransitionTargetNode::Terminal
                                    | psi_typed_trees::statement::TransitionTargetNode::Value(_)
                            )
                        {
                            exits.append(ContractExitFact {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                                statement_index,
                                transition_target: target,
                                ensures,
                            });
                        }
                    }
                }
                // Typed dispatches are exhaustive; later statements beyond
                // this maximal run are not additional normal return sites.
                if transition.continuation.is_valid()
                    || !matches!(
                        statements.get(statement_index + 1),
                        Some(StatementNode::Transition(_))
                    )
                {
                    break;
                }
            }
            if !has_transition {
                exits.append(ContractExitFact {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    statement_index: if matches!(
                        statements.last(),
                        Some(StatementNode::Expression(_))
                    ) {
                        statements.len() - 1
                    } else {
                        statements.len()
                    },
                    transition_target: Default::default(),
                    ensures,
                });
            }
        }
    }

    exits
}

fn append_contract_fact_refs(
    contract_facts: &psi_arena::Arena<ContractProofFact>,
    fact_refs: &mut psi_arena::Arena<ContractProofFactRef>,
    machine_symbol: SymbolHandle,
    state_symbol: Option<SymbolHandle>,
    kind: ContractProofFactKind,
    include_machine_contracts: bool,
) -> HandleSpan<ContractProofFactRef> {
    let mut span = HandleSpan::empty();

    for (handle, fact) in contract_facts.iter() {
        let owner_matches = match fact.owner {
            ContractProofFactOwner::Machine {
                machine_symbol: owner_symbol,
            } => include_machine_contracts && owner_symbol == machine_symbol,
            ContractProofFactOwner::MachineState {
                machine_symbol: owner_machine_symbol,
                state_symbol: owner_state_symbol,
            } => {
                owner_machine_symbol == machine_symbol
                    && state_symbol.is_some_and(|state_symbol| state_symbol == owner_state_symbol)
            }
            // A trait machine signature's contracts (boundary trait `requires`/
            // `ensures`) are owned by the trait symbol plus the signature symbol;
            // calls through trait-typed receivers target exactly that pair.
            ContractProofFactOwner::StateSignature {
                owner_symbol,
                state_symbol: owner_state_symbol,
            } => {
                owner_symbol == machine_symbol
                    && state_symbol.is_some_and(|state_symbol| state_symbol == owner_state_symbol)
            }
            ContractProofFactOwner::Unknown
            | ContractProofFactOwner::OperatorDeclaration { .. }
            | ContractProofFactOwner::OperatorUse { .. } => false,
        };

        if owner_matches && fact.kind == kind {
            fact_refs.append_to_span(&mut span, ContractProofFactRef { fact: handle });
        }
    }

    span
}

pub(crate) fn contract_target_from_state_symbol(
    program: &psi_typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    if !target_state_symbol.is_valid() {
        return None;
    }

    if let Some(target_machine) = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target_state_symbol)
    }) {
        return Some((target_machine.symbol, target_state_symbol));
    }

    if program
        .machine_parameter_signature(target_state_symbol)
        .is_some()
    {
        // The parameter symbol owns its callable contract. Using the
        // declaring machine here would also select that machine's entry
        // requires as if they belonged to the callee.
        return Some((target_state_symbol, target_state_symbol));
    }

    // A call through a trait-typed receiver targets a trait machine signature
    // rather than a machine state; the owning trait stands in for the machine
    // so the signature's requires/ensures contracts attach to the call.
    if let Some(target_trait) = program.traits().iter().find(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .any(|signature| signature.symbol == target_state_symbol)
    }) {
        return Some((target_trait.symbol, target_state_symbol));
    }

    None
}
