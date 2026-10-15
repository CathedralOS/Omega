use language_semantics::{MachineSupplyMode, TerminationGuarantee};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;

use super::BuildTimeCallEdge;

/// `discharge_authored_requires` records ordinary contract checking of either
/// the concrete invocation at snapshot arguments or the internal call sites
/// behind an unconditional entry. The latter can reuse the same checked program.
/// The closure walk stands down on that axis alone; termination, linear
/// carriers, and the other floor obligations keep applying unchanged.
pub(super) fn checked_closure_violation(
    call_edges: &[BuildTimeCallEdge],
    program: &TypedTrees,
    root: &Machine,
    discharge_authored_requires: bool,
) -> Option<String> {
    let mut completed = Vec::new();
    let mut active = Vec::new();
    let mut path = Vec::new();
    machine_termination_violation(
        call_edges,
        program,
        root.symbol,
        &mut completed,
        &mut active,
        &mut path,
        discharge_authored_requires,
    )
}

/// Whether `root`'s call-edge closure carries any authored `requires` premise —
/// on a reachable machine, one of its states, or a callable signature target.
/// The traversal mirrors `machine_termination_violation`'s edge following: an
/// unmeasured terminal recursion edge keeps its machine symbol here as well.
pub(super) fn closure_has_authored_requires(
    call_edges: &[BuildTimeCallEdge],
    program: &TypedTrees,
    root: SymbolHandle,
) -> bool {
    each_closure_machine(
        call_edges,
        program,
        root,
        |machine| {
            (has_authored_requires(program.machine_contracts(machine))
                || program
                    .machine_states(machine)
                    .iter()
                    .any(|state| has_authored_requires(program.state_contracts(state))))
            .then_some(())
        },
        |callable| callable_has_authored_requires(program, callable).then_some(()),
    )
    .is_some()
}

/// Whether every authored `requires` premise in `root`'s call-edge closure is
/// a parameter-domain premise on one of `root`'s own states. Such a premise
/// is the typed lowering's `value in Domain` obligation for a domain-qualified
/// parameter; an invocation that has already proved that exact membership for
/// each concrete argument owes nothing else, so it may take the concrete-
/// premise route. Any authored `requires` clause, a premise on a callee or a
/// callable target, or a premise naming a parameter whose type does not carry
/// that domain keeps the conservative fence. The traversal mirrors
/// `closure_has_authored_requires`.
pub(super) fn closure_requires_are_root_parameter_domains(
    call_edges: &[BuildTimeCallEdge],
    program: &TypedTrees,
    root: SymbolHandle,
) -> bool {
    each_closure_machine(
        call_edges,
        program,
        root,
        |machine| {
            if has_authored_requires(program.machine_contracts(machine)) {
                return Some(());
            }
            for state in program.machine_states(machine) {
                for contract in program.state_contracts(state) {
                    if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
                        continue;
                    }
                    if machine.symbol != root
                        || !is_parameter_domain_premise(program, state, contract)
                    {
                        return Some(());
                    }
                }
            }
            None
        },
        |callable| callable_has_authored_requires(program, callable).then_some(()),
    )
    .is_none()
}

/// The call-edge-closure walk both premise checks share: a DFS from `root`
/// through `call_edges` where each visited machine first answers `machine`
/// (Some = the walk's outcome) and each non-machine callable edge target
/// answers `callable`. An unmeasured terminal recursion edge keeps its
/// machine symbol; anything else goes through `callable`.
fn each_closure_machine(
    call_edges: &[BuildTimeCallEdge],
    program: &TypedTrees,
    root: SymbolHandle,
    mut machine: impl FnMut(&Machine) -> Option<()>,
    mut callable: impl FnMut(SymbolHandle) -> Option<()>,
) -> Option<()> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(machine_symbol) = pending.pop() {
        if visited.contains(&machine_symbol) {
            continue;
        }
        visited.push(machine_symbol);
        let Some(found) = program.machine(machine_symbol) else {
            continue;
        };
        if machine(found).is_some() {
            return Some(());
        }
        for call in call_edges
            .iter()
            .filter(|call| call.source_machine_symbol == machine_symbol)
        {
            let target_machine_symbol = if call.target_machine_symbol.is_valid() {
                Some(call.target_machine_symbol)
            } else if call.target_state_symbol.is_valid()
                && program.symbols.get(call.target_state_symbol).kind == SymbolKind::Machine
            {
                Some(call.target_state_symbol)
            } else {
                None
            };
            match target_machine_symbol {
                Some(target) => pending.push(target),
                None => {
                    if callable(call.target_state_symbol).is_some() {
                        return Some(());
                    }
                }
            }
        }
    }
    None
}

/// A generated (keyword-less, unbound) premise whose every fact is the
/// membership of one of `state`'s parameters in an argument-free declared
/// domain that the parameter's own type carries.
fn is_parameter_domain_premise(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    contract: &typed_trees::signature::SignatureContract,
) -> bool {
    use typed_trees::types::{DomainConstraintSubject, TypeConstraintNode, TypeReferenceNode};

    if contract.keyword_source_span.is_some() || contract.binding.is_some() {
        return false;
    }
    let facts = program.proof_facts.span_or_empty(contract.facts);
    !facts.is_empty()
        && facts.iter().all(|fact| {
            let Some(membership) = fact.as_membership() else {
                return false;
            };
            if !membership.domain_symbol.is_valid()
                || !membership.domain_arguments.is_empty()
                || !program
                    .expression_table
                    .expression_is_valid(membership.value)
            {
                return false;
            }
            let Some(path) = program.expression_table.name_path_node(membership.value) else {
                return false;
            };
            let Some(parameter) = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol.is_valid() && parameter.symbol == path.symbol)
            else {
                return false;
            };
            let mut reference = parameter.type_reference;
            let mut visited = Vec::new();
            while let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_table.type_reference(reference)
            {
                if visited.contains(&reference) {
                    return false;
                }
                visited.push(reference);
                if program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| {
                        matches!(
                            constraint,
                            TypeConstraintNode::Domain(domain)
                                if domain.subject == DomainConstraintSubject::Declared
                                    && domain.symbol == membership.domain_symbol
                                    && domain.arguments.is_empty()
                        )
                    })
                {
                    return true;
                }
                reference = *base_type;
            }
            false
        })
}

fn machine_termination_violation(
    call_edges: &[BuildTimeCallEdge],
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    completed: &mut Vec<SymbolHandle>,
    active: &mut Vec<SymbolHandle>,
    path: &mut Vec<String>,
    discharge_authored_requires: bool,
) -> Option<String> {
    if completed.contains(&machine_symbol) {
        return None;
    }
    let machine = program.machine(machine_symbol)?;
    path.push(machine.name.as_str().to_owned());

    if !discharge_authored_requires
        && let Some(violation) = machine_precondition_violation(program, machine, path)
    {
        path.pop();
        return Some(violation);
    }
    if let Some(violation) = machine_linear_carrier_violation(program, machine, path) {
        path.pop();
        return Some(violation);
    }

    if active.contains(&machine_symbol) {
        // Re-entering an active machine is a recursive cycle. A machine whose
        // authored `terminates by` measure already produced a checked
        // `Terminates` summary has an ordinary termination proof, so the cycle
        // is discharged instead of reported.
        let measured = machine.supply_mode == MachineSupplyMode::CheckedBody
            && matches!(
                typed_trees_to_checked_trees::infer_machine_termination_summary(
                    program,
                    machine_symbol
                ),
                Some(TerminationGuarantee::Terminates { .. })
            );
        if !measured {
            let violation = format!(
                "recursive machine-call cycle has no ordinary termination proof along `{}`",
                path.join(" -> ")
            );
            path.pop();
            return Some(violation);
        }
        path.pop();
        return None;
    }
    active.push(machine_symbol);

    let locally_terminates = machine.supply_mode == MachineSupplyMode::CheckedBody
        && matches!(
            typed_trees_to_checked_trees::infer_machine_termination_summary(
                program,
                machine_symbol
            ),
            Some(TerminationGuarantee::Terminates { .. })
        );
    if !locally_terminates {
        // A requirement slot has no body by design: reaching it without a
        // selected provider is a missing selection, not a termination defect.
        let violation = if matches!(
            machine.supply_mode,
            MachineSupplyMode::Requirement
                | MachineSupplyMode::TopLevelRequirement
                | MachineSupplyMode::Boundary
        ) {
            format!(
                "machine call path `{}` reaches a requirement that requires exact authored selection before evaluation; the evaluator cannot execute a bodyless requirement",
                path.join(" -> ")
            )
        } else {
            format!(
                "machine call path `{}` has no ordinary checked `Terminates` guarantee",
                path.join(" -> ")
            )
        };
        active.retain(|active_symbol| *active_symbol != machine_symbol);
        path.pop();
        return Some(violation);
    }

    for call in call_edges
        .iter()
        .filter(|call| call.source_machine_symbol == machine_symbol)
    {
        let target_machine_symbol = if call.target_machine_symbol.is_valid() {
            Some(call.target_machine_symbol)
        } else if call.target_state_symbol.is_valid()
            && program.symbols.get(call.target_state_symbol).kind == SymbolKind::Machine
        {
            // Unmeasured terminal recursion deliberately remains a
            // machine-symbol call until validation can diagnose the
            // missing measure. Semantic evaluation runs earlier,
            // so its admission closure must retain that edge too.
            Some(call.target_state_symbol)
        } else {
            None
        };
        if let Some(target_machine_symbol) = target_machine_symbol {
            if let Some(violation) = machine_termination_violation(
                call_edges,
                program,
                target_machine_symbol,
                completed,
                active,
                path,
                discharge_authored_requires,
            ) {
                active.retain(|active_symbol| *active_symbol != machine_symbol);
                path.pop();
                return Some(violation);
            }
        } else if let Some(violation) = callable_contract_violation(
            program,
            call.target_state_symbol,
            path,
            discharge_authored_requires,
        ) {
            active.retain(|active_symbol| *active_symbol != machine_symbol);
            path.pop();
            return Some(violation);
        }
    }

    active.retain(|active_symbol| *active_symbol != machine_symbol);
    completed.push(machine_symbol);
    path.pop();
    None
}

fn machine_precondition_violation(
    program: &TypedTrees,
    machine: &Machine,
    path: &[String],
) -> Option<String> {
    if has_authored_requires(program.machine_contracts(machine)) {
        return Some(format!(
            "machine `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
            machine.name,
            path.join(" -> ")
        ));
    }
    program.machine_states(machine).iter().find_map(|state| {
        has_authored_requires(program.state_contracts(state)).then(|| {
            format!(
                "state `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
                state.name,
                path.join(" -> ")
            )
        })
    })
}

pub(super) fn has_authored_requires(
    contracts: &[typed_trees::signature::SignatureContract],
) -> bool {
    contracts
        .iter()
        .any(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
}

fn machine_linear_carrier_violation(
    program: &TypedTrees,
    machine: &Machine,
    path: &[String],
) -> Option<String> {
    let describe = |context: &str, type_reference| {
        (program.type_multiplicity(type_reference)
            == language_semantics::Multiplicity::Linear)
            .then(|| {
                format!(
                    "{context} has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
                    program.display_type_reference(type_reference),
                    path.join(" -> ")
                )
            })
    };

    if let Some(attached_data) = machine.attached_data.as_ref()
        && program
            .attached_data_definition(machine)
            .is_some_and(|definition| {
                definition.properties.multiplicity == language_semantics::Multiplicity::Linear
            })
    {
        return Some(format!(
            "machine instance `{}` has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
            machine.name,
            attached_data,
            path.join(" -> ")
        ));
    }

    for owned in program.machine_owned_data(machine) {
        if let Some(violation) = describe(
            &format!("machine-owned value `{}`", owned.name),
            owned.type_reference,
        ) {
            return Some(violation);
        }
    }

    for state in program.machine_states(machine) {
        for parameter in program.state_parameters(state) {
            if let Some(violation) = describe(
                &format!("state `{}` parameter `{}`", state.name, parameter.name),
                parameter.type_reference,
            ) {
                return Some(violation);
            }
        }
        if let Some(violation) =
            describe(&format!("state `{}` result", state.name), state.return_type)
        {
            return Some(violation);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            let Some(local) = statement.as_local_data() else {
                continue;
            };
            if let Some(violation) = describe(
                &format!("state `{}` local `{}`", state.name, local.name),
                local.type_reference,
            ) {
                return Some(violation);
            }
        }
    }

    None
}

fn callable_signature(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::signature::StateSignature> {
    if !symbol.is_valid()
        || matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::BuiltinFunction | SymbolKind::Operator
        )
    {
        return None;
    }
    program
        .machine_parameter_signature(symbol)
        .map(|(_, signature)| signature)
        .or_else(|| {
            program
                .trait_machine_signature(symbol)
                .map(|(_, signature)| signature)
        })
}

fn callable_has_authored_requires(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    callable_signature(program, symbol).is_some_and(|signature| {
        has_authored_requires(program.state_signature_contracts(signature))
    })
}

fn callable_contract_violation(
    program: &TypedTrees,
    symbol: SymbolHandle,
    path: &[String],
    discharge_authored_requires: bool,
) -> Option<String> {
    let signature = callable_signature(program, symbol)?;

    if !discharge_authored_requires
        && has_authored_requires(program.state_signature_contracts(signature))
    {
        return Some(format!(
            "callable contract `{}` has an authored `requires` premise along `{}`; pre-check semantic evaluation has no checked invocation proof for that premise",
            signature.name,
            path.join(" -> ")
        ));
    }

    for parameter in program.state_signature_parameters(signature) {
        if program.type_multiplicity(parameter.type_reference)
            == language_semantics::Multiplicity::Linear
        {
            return Some(format!(
                "callable contract `{}` parameter `{}` has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
                signature.name,
                parameter.name,
                program.display_type_reference(parameter.type_reference),
                path.join(" -> ")
            ));
        }
    }
    if program.type_multiplicity(signature.return_type) == language_semantics::Multiplicity::Linear
    {
        return Some(format!(
            "callable contract `{}` result has linear runtime type `{}` along `{}`; semantic evaluation has no proof/build-admission for that resource carrier",
            signature.name,
            program.display_type_reference(signature.return_type),
            path.join(" -> ")
        ));
    }

    if signature.termination_guarantee.promises_termination() {
        return None;
    }

    let name = signature.name.as_str();
    Some(format!(
        "callable contract `{name}` reached from `{}` publishes no `Terminates` guarantee",
        path.join(" -> ")
    ))
}
