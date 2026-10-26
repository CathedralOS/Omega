use validation::machine_state_count;

use arena::HandleSpan;
use checked_trees::statement::StatementNode;
use checked_trees::{
    BorrowFacts, ContractCallFact, ContractExitFact, ContractProofFact, ContractProofFactKind,
    ContractProofFactOwner, ContractProofFactRef,
};
use symbols::SymbolHandle;

pub(crate) fn build_contract_call_facts(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    contract_facts: &mut arena::Arena<ContractProofFact>,
    evidence_terms: &mut arena::Arena<checked_trees::CheckedEvidenceTerm>,
) -> Result<
    (
        arena::Arena<ContractProofFactRef>,
        arena::Arena<ContractCallFact>,
        arena::Arena<checked_trees::CallContractSignature>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    let mut fact_refs = arena::Arena::with_capacity(contract_facts.len());
    let mut calls = arena::Arena::with_capacity(borrow.calls.len());
    let mut signatures = arena::Arena::with_capacity(borrow.calls.len());

    for state in borrow.states.iter().map(|(_, state)| state) {
        for call in borrow.calls.span_or_empty(state.calls) {
            let Some(contract_owner) =
                contract_target_from_state_symbol(program, call.target_symbol)
            else {
                continue;
            };
            // A selector call's operands pair with its slot's roster, selector
            // first; the requirement the slot instantiates owns the contract.
            // Naming the requirement as the target would pair its formals with
            // operands one position early.
            let (target_machine_symbol, target_state_symbol) =
                if program.dynamic_slot_by_symbol(call.target_symbol).is_some() {
                    (call.target_symbol, call.target_symbol)
                } else {
                    contract_owner
                };

            let call_site = validation::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            );
            if call_site.is_none()
                && validation::selected_requirement_signature(program, call.target_symbol).is_some()
            {
                return Err(vec![diagnostics::Diagnostic::error(
                    "public requirement contract has no exact source invocation",
                )]);
            }
            let signature = if let Some(site) = &call_site {
                super::call_signatures::retain(
                    program,
                    &mut signatures,
                    contract_facts,
                    evidence_terms,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                    site,
                )
                .map_err(|error| vec![error])?
            } else {
                Default::default()
            };
            let signature_owner = super::call_signatures::owner(&signatures, signature);
            let contract_target = call_site
                .as_ref()
                .and_then(validation::CallSite::static_requirement_dispatch)
                .map(|dispatch| (dispatch.declaring_trait, dispatch.contract_symbol()))
                .or_else(|| {
                    // The binder owns the contract selection; its public
                    // signature owns the formals and reserved result. Keep
                    // this logical frame distinct from the executable target
                    // coordinates above, including for named terminal calls.
                    program
                        .machine_parameter_signature(call.target_symbol)
                        .map(|(_, signature)| (call.target_symbol, signature.symbol))
                })
                .unwrap_or(contract_owner);
            append_contract_call(
                contract_facts,
                &mut fact_refs,
                &mut calls,
                call_site.as_ref().is_some_and(|site| {
                    !validation::call_site_evidence_arguments(site).is_empty()
                        || !crate::semantic::calls::call_parameter_domains(
                            program,
                            state.machine_symbol,
                            state.state_symbol,
                            target_machine_symbol,
                            target_state_symbol,
                            site,
                        )
                        .is_empty()
                }),
                ContractCallSite {
                    caller_machine_symbol: state.machine_symbol,
                    caller_state_symbol: state.state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target_machine_symbol,
                    target_state_symbol,
                    contract_machine_symbol: contract_target.0,
                    contract_state_symbol: contract_target.1,
                    signature,
                    signature_owner,
                    targets_machine_entry: program.machines().iter().any(|machine| {
                        machine.symbol == target_machine_symbol
                            && program
                                .machine_states(machine)
                                .first()
                                .is_some_and(|entry| entry.symbol == target_state_symbol)
                    }),
                    // Only an internal jump keeps the current invocation's
                    // machine contract package. A named tail call to another
                    // machine still owes that callee's entry requirements.
                    is_state_transfer: call_site.as_ref().is_some_and(|site| {
                        site.is_internal_state_transfer(program, state.machine_symbol)
                    }),
                },
            );
        }
    }

    Ok((fact_refs, calls, signatures))
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
    signature: arena::Handle<checked_trees::CallContractSignature>,
    signature_owner: Option<ContractProofFactOwner>,
    targets_machine_entry: bool,
    is_state_transfer: bool,
}

fn append_contract_call(
    contract_facts: &arena::Arena<ContractProofFact>,
    fact_refs: &mut arena::Arena<ContractProofFactRef>,
    calls: &mut arena::Arena<ContractCallFact>,
    has_call_obligations: bool,
    site: ContractCallSite,
) {
    let requires = append_contract_fact_refs(
        contract_facts,
        fact_refs,
        site.contract_machine_symbol,
        Some(site.contract_state_symbol),
        ContractProofFactKind::Requires,
        // Entry backedges renew machine preconditions even though they do not
        // return and cannot import the machine's postconditions.
        !site.is_state_transfer || site.targets_machine_entry,
        site.signature_owner,
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
            site.signature_owner,
        )
    };

    if requires.is_empty() && ensures.is_empty() && !has_call_obligations {
        return;
    }

    calls.append(ContractCallFact {
        caller_machine_symbol: site.caller_machine_symbol,
        caller_state_symbol: site.caller_state_symbol,
        statement_index: site.statement_index,
        call_ordinal: site.call_ordinal,
        target_machine_symbol: site.target_machine_symbol,
        target_state_symbol: site.target_state_symbol,
        signature: site.signature,
        requires,
        ensures,
        evidence_arguments: HandleSpan::empty(),
    });
}

pub(crate) fn build_contract_exit_facts(
    program: &dyn ::validation::CheckingInput,
    contract_facts: &arena::Arena<ContractProofFact>,
    fact_refs: &mut arena::Arena<ContractProofFactRef>,
) -> arena::Arena<ContractExitFact> {
    let mut exits = arena::Arena::with_capacity(machine_state_count(program));

    for machine in program.machines() {
        if !machine.body_is_present {
            continue;
        }
        // Every normal return retains its live context. Cleanup and other
        // body obligations exist even without an authored postcondition.
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            let ensures = append_contract_fact_refs(
                contract_facts,
                fact_refs,
                machine.symbol,
                Some(state.symbol),
                ContractProofFactKind::Ensures,
                true,
                None,
            );

            let mut has_transition = false;
            for (statement_index, statement) in statements.iter().enumerate() {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                has_transition = true;
                if transition.exit == typed_trees::statement::TransitionExit::Ordinary {
                    for target in transition.targets() {
                        let returns_from_machine =
                            match program.statement_table.transition_target(target) {
                                typed_trees::statement::TransitionTargetNode::Terminal
                                | typed_trees::statement::TransitionTargetNode::Value(_) => true,
                                typed_trees::statement::TransitionTargetNode::Named {
                                    path,
                                    static_machine_parameter,
                                    arguments,
                                    evidence_arguments,
                                    source_span,
                                    authored_call_selection,
                                    ..
                                } => {
                                    let site = validation::CallSite::TransitionNamed {
                                        path,
                                        static_machine_parameter: *static_machine_parameter,
                                        arguments: *arguments,
                                        evidence_arguments,
                                        source_span: *source_span,
                                        authored_call_selection: *authored_call_selection,
                                    };
                                    // Recursive machine calls complete this
                                    // invocation too. Only an actual internal
                                    // arrival keeps its postconditions pending.
                                    contract_target_from_state_symbol(program, path.symbol)
                                        .is_some()
                                        && !site.is_internal_state_transfer(program, machine.symbol)
                                }
                            };
                        if returns_from_machine {
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
    contract_facts: &arena::Arena<ContractProofFact>,
    fact_refs: &mut arena::Arena<ContractProofFactRef>,
    machine_symbol: SymbolHandle,
    state_symbol: Option<SymbolHandle>,
    kind: ContractProofFactKind,
    include_machine_contracts: bool,
    signature_owner: Option<ContractProofFactOwner>,
) -> HandleSpan<ContractProofFactRef> {
    let mut span = HandleSpan::empty();

    for (handle, fact) in contract_facts.iter() {
        let owner_matches = if let Some(owner) = signature_owner {
            fact.owner == owner
        } else {
            match fact.owner {
                ContractProofFactOwner::Machine {
                    machine_symbol: owner_symbol,
                } => include_machine_contracts && owner_symbol == machine_symbol,
                ContractProofFactOwner::MachineState {
                    machine_symbol: owner_machine_symbol,
                    state_symbol: owner_state_symbol,
                } => {
                    owner_machine_symbol == machine_symbol
                        && state_symbol
                            .is_some_and(|state_symbol| state_symbol == owner_state_symbol)
                }
                // A trait machine signature's contracts (boundary trait `requires`/
                // `ensures`) are owned by the trait symbol plus the signature symbol;
                // calls through trait-typed receivers target exactly that pair.
                ContractProofFactOwner::StateSignature {
                    owner_symbol,
                    state_symbol: owner_state_symbol,
                } => {
                    owner_symbol == machine_symbol
                        && state_symbol
                            .is_some_and(|state_symbol| state_symbol == owner_state_symbol)
                }
                ContractProofFactOwner::Unknown
                | ContractProofFactOwner::CallSignature { .. }
                | ContractProofFactOwner::OperatorDeclaration { .. }
                | ContractProofFactOwner::OperatorUse { .. } => false,
            }
        };

        if owner_matches && fact.kind == kind {
            fact_refs.append_to_span(&mut span, ContractProofFactRef { fact: handle });
        }
    }

    span
}

pub(crate) fn contract_target_from_state_symbol(
    program: &dyn ::validation::CheckingInput,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    if !target_state_symbol.is_valid() {
        return None;
    }

    // A selector call targets its instantiated slot; the requirement it
    // instantiates owns the call's contract, as for any requirement call.
    if let Some(slot) = program.dynamic_slot_by_symbol(target_state_symbol) {
        return Some((slot.declaring_trait, slot.requirement));
    }

    // Named calls may carry the machine symbol rather than its entry-state
    // symbol. Resolve both forms to the same exact invocation contract.
    if let Some(machine) = program.machine_by_symbol(target_state_symbol) {
        let entry = program.machine_states(machine).first()?;
        return Some((machine.symbol, entry.symbol));
    }

    if let Some((target_machine, _)) = validation::find_state(program, target_state_symbol) {
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

    if let Some((declaration, signature)) =
        validation::selected_requirement_signature(program, target_state_symbol)
    {
        return Some((declaration.symbol, signature.symbol));
    }

    None
}
