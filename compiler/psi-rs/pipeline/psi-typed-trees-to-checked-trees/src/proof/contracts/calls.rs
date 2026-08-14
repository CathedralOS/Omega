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

            append_contract_call(
                contract_facts,
                &mut fact_refs,
                &mut calls,
                crate::find_call_site(
                    program,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                )
                .is_some_and(|site| !crate::call_site_evidence_arguments(&site).is_empty()),
                ContractCallSite {
                    caller_machine_symbol: state.machine_symbol,
                    caller_state_symbol: state.state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target_machine_symbol,
                    target_state_symbol,
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
        site.target_machine_symbol,
        Some(site.target_state_symbol),
        ContractProofFactKind::Requires,
    );
    let ensures = append_contract_fact_refs(
        contract_facts,
        fact_refs,
        site.target_machine_symbol,
        Some(site.target_state_symbol),
        ContractProofFactKind::Ensures,
    );

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
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            let Some((statement_index, StatementNode::Expression(_))) =
                statements.iter().enumerate().next_back()
            else {
                continue;
            };
            let ensures = append_contract_fact_refs(
                contract_facts,
                fact_refs,
                machine.symbol,
                Some(state.symbol),
                ContractProofFactKind::Ensures,
            );

            if ensures.is_empty() {
                continue;
            }

            exits.append(ContractExitFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
                ensures,
            });
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
) -> HandleSpan<ContractProofFactRef> {
    let mut span = HandleSpan::empty();

    for (handle, fact) in contract_facts.iter() {
        let owner_matches = match fact.owner {
            ContractProofFactOwner::Machine {
                machine_symbol: owner_symbol,
            } => owner_symbol == machine_symbol,
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
            ContractProofFactOwner::Unknown | ContractProofFactOwner::OperatorUse { .. } => false,
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
