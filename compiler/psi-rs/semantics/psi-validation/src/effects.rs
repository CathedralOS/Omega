use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::statement::StatementNode;

pub fn validate_behavior_plan(
    program: &TypedTrees,
    operational: &psi_effects::OperationalPlan,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let service_reaches = psi_effects::infer_service_reaches(program, operational);

    validate_pure_discards(program, operational, &service_reaches, &mut diagnostics);
    validate_asm_intrinsic_declarations(program, operational, &service_reaches, &mut diagnostics);
    validate_service_reach_ceilings(program, &service_reaches, &mut diagnostics);

    for machine_summary in operational.machines() {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_summary.symbol)
        else {
            continue;
        };

        if machine.attached_data.is_some() && machine.name.as_str().ends_with("::drop") {
            if machine.suspends
                || machine.blocks
                || machine_summary.transitive_may_suspend
                || machine_summary.transitive_may_block
            {
                diagnostics.push(Diagnostic::error(format!(
                    "cleanup machine `{}` must be transitively non-suspending and nonblocking",
                    machine.name
                )));
            }
        }

        // Requirements, boundaries, accepted contracts, and external
        // realizations publish closed operational ceilings. Omission is an
        // authored `false`, unlike omission on a private checked body, where
        // both axes are inferred. Validate the declaration-free body fixed
        // points so a published machine cannot launder behavior through a
        // local helper or a pinned requirement.
        if machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody {
            if machine_summary.body_may_suspend && !machine.suspends {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` omits `suspends;` from its published contract, but its body may suspend",
                    machine.name
                )));
            }
            if machine_summary.body_may_block && !machine.blocks {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` omits `blocks;` from its published contract, but its body may block",
                    machine.name
                )));
            }
        }
    }

    crate::finish_diagnostics(diagnostics)
}

fn validate_service_reach_ceilings(
    program: &TypedTrees,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        let publishes = machine.supply_mode
            != psi_language_semantics::MachineSupplyMode::CheckedBody
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty();
        if !publishes {
            continue;
        }
        let Some(summary) = service_reaches.for_machine(machine.symbol) else {
            continue;
        };
        let published = service_reaches.services(summary.published);
        let missing = service_reaches
            .services(summary.inferred_transitive)
            .iter()
            .copied()
            .filter(|service| !published.contains(service))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            continue;
        }
        let names = missing
            .iter()
            .map(|service| {
                program
                    .service_reaches
                    .definition(*service)
                    .map(|definition| definition.name.as_str())
                    .unwrap_or("<unknown canonical service>")
            })
            .collect::<Vec<_>>();
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` publishes service reach `{}` but its checked body reaches undeclared service{} `{}`",
            machine.name,
            format_service_row(program, published),
            if names.len() == 1 { "" } else { "s" },
            names.join(" + "),
        )));
    }
}

fn format_service_row(
    program: &TypedTrees,
    services: &[psi_language_semantics::ServiceReachId],
) -> String {
    if services.is_empty() {
        return "<none>".to_owned();
    }
    services
        .iter()
        .map(|service| {
            program
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.as_str())
                .unwrap_or("<unknown canonical service>")
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

mod asm_discharge;
pub use asm_discharge::validate_asm_discharge;
use asm_discharge::validate_asm_intrinsic_declarations;

/// DECISION 12 -- DISCARD ADMITS EFFECTS: `_ = call();` exists to drop a
/// result the caller does not need while keeping the callee's observable work.
/// When the resolved callee provably has no observable channel at all -- an
/// empty service/operational reach AND no `&mut`/out parameters -- the statement is dead
/// runtime code. AMENDED (owner, 2026-07-12) from reject to WARN, for
/// uniform compilation: the statement always compiles, and the deadness
/// surfaces as a warning ONLY outside proof use. Proof use is silent by
/// design -- a statement call whose callee is a PROOF MACHINE is a CITATION
/// (ch10 "Citing Proofs": invoked for its ensures, erased at codegen), and
/// inside a proof machine's own body every discard is compile-time
/// material. Service reach and the independent operational summaries come
/// from their recursive plans, never the lowercase compatibility set.
fn validate_pure_discards(
    program: &TypedTrees,
    operational: &psi_effects::OperationalPlan,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let classification = psi_typed_trees::proof_only::classify(program);
    for machine in program.machines() {
        // Inside a proof machine, discards are proof context -- silent.
        if classification.is_proof_machine(program, machine) {
            continue;
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::Call(call) = statement else {
                    continue;
                };
                if !call.discards_result {
                    continue;
                }

                // A proof-machine callee makes the statement a CITATION,
                // not dead code.
                if machine_owning_state(program, call.target_symbol)
                    .is_some_and(|callee| classification.is_proof_machine(program, callee))
                {
                    continue;
                }

                // An unresolved callee cannot be PROVABLY pure; resolution
                // errors are owned by other passes.
                let Some(callee) = resolve_discard_callee(
                    program,
                    operational,
                    service_reaches,
                    call.target_symbol,
                ) else {
                    continue;
                };

                if !callee.has_service_reach
                    && !callee.may_suspend
                    && !callee.may_block
                    && !callee.has_mutable_parameter
                {
                    diagnostics.push(Diagnostic::warning(format!(
                        "`_ = {target}(...);` discards a provably pure call: `{target}` has no service reach, suspension, blocking, or mutable out-parameters, so the discard is dead code; remove the statement or use the result",
                        target = call.target.as_str()
                    )));
                }
            }
        }
    }
}

struct DiscardCallee {
    has_service_reach: bool,
    may_suspend: bool,
    may_block: bool,
    has_mutable_parameter: bool,
}

/// The machine whose states include the called state -- the discard rules
/// reason about the CALLEE MACHINE (purity surface, proof classification),
/// while the call site records only the state symbol.
fn machine_owning_state(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<&psi_typed_trees::machine::Machine> {
    if !target_symbol.is_valid() {
        return None;
    }
    program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target_symbol)
    })
}

fn resolve_discard_callee(
    program: &TypedTrees,
    operational: &psi_effects::OperationalPlan,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
    target_symbol: SymbolHandle,
) -> Option<DiscardCallee> {
    if !target_symbol.is_valid() {
        return None;
    }

    for machine in program.machines() {
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target_symbol)
        else {
            continue;
        };

        // The callee machine's transitive set is the conservative surface: a
        // called state may transition through sibling states, so anything the
        // machine can reach counts against purity.
        let operational_may = operational
            .machines()
            .iter()
            .find(|entry| entry.symbol == machine.symbol)
            .map(|entry| (entry.transitive_may_suspend, entry.transitive_may_block))?;

        return Some(DiscardCallee {
            has_service_reach: service_reaches
                .for_machine(machine.symbol)
                .is_some_and(|summary| {
                    !service_reaches
                        .services(summary.inferred_transitive)
                        .is_empty()
                }),
            may_suspend: operational_may.0,
            may_block: operational_may.1,
            has_mutable_parameter: program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_mutable),
        });
    }

    for trait_definition in program.traits() {
        if let Some(signature) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
        {
            return Some(signature_discard_callee(
                program,
                trait_definition,
                signature,
            ));
        }
    }

    None
}

/// Signatures have no body to traverse. Their normalized service row and, for
/// a boundary trait, the enclosing service identity still make the call
/// observable without consulting a lowercase compatibility list.
fn signature_discard_callee(
    program: &TypedTrees,
    trait_definition: &psi_typed_trees::trait_definition::TraitDefinition,
    signature: &psi_typed_trees::signature::StateSignature,
) -> DiscardCallee {
    DiscardCallee {
        has_service_reach: trait_definition.is_boundary
            || !program
                .service_reach_rows
                .services(signature.service_reach_row)
                .is_empty(),
        may_suspend: signature.suspends,
        may_block: signature.blocks,
        has_mutable_parameter: program
            .state_signature_parameters(signature)
            .iter()
            .any(|parameter| parameter.is_mutable),
    }
}

pub(super) fn append_effect_call_step(
    program: &TypedTrees,
    message: &mut String,
    depth: usize,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    target_machine_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
) {
    message.push('\n');
    push_indent(message, depth);
    message.push_str(&callable_state_label(
        program,
        caller_machine_symbol,
        caller_state_symbol,
    ));
    message.push_str(" statement ");
    message.push_str(&statement_index.to_string());
    message.push_str(" call ");
    message.push_str(&call_ordinal.to_string());
    message.push_str(" -> ");
    message.push_str(&call_target_label(
        program,
        target_machine_symbol,
        target_state_symbol,
    ));
}

fn push_indent(message: &mut String, depth: usize) {
    for _ in 0..depth {
        message.push_str("  ");
    }
}

fn machine_label(program: &TypedTrees, symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .map(|machine| machine.name.to_string())
        .unwrap_or_else(|| symbol_label(symbol))
}

fn state_label(program: &TypedTrees, symbol: SymbolHandle) -> String {
    find_machine_state(program, symbol)
        .map(|state| state.name.to_string())
        .or_else(|| find_signature_name(program, symbol))
        // Builtin targets (the `asm#...` intrinsics) resolve through the
        // symbol table -- the effect path should read `asm#hlt`, not a raw
        // handle.
        .or_else(|| {
            symbol
                .is_valid()
                .then(|| program.symbols.name(symbol))
                .filter(|name| !name.is_empty())
                .map(|name| name.to_owned())
        })
        .unwrap_or_else(|| symbol_label(symbol))
}

fn call_target_label(
    program: &TypedTrees,
    target_machine_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
) -> String {
    if target_machine_symbol.is_valid() {
        return callable_state_label(program, target_machine_symbol, target_state_symbol);
    }

    state_label(program, target_state_symbol)
}

fn callable_state_label(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> String {
    let machine = machine_label(program, machine_symbol);
    let state = state_label(program, state_symbol);

    if machine
        .rsplit_once("::")
        .is_some_and(|(_, entry_name)| entry_name == state)
    {
        machine
    } else {
        format!("{machine}::{state}")
    }
}

fn find_machine_state(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&psi_typed_trees::state::State> {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|state| state.symbol == symbol)
}

fn find_signature_name(program: &TypedTrees, symbol: SymbolHandle) -> Option<String> {
    for trait_definition in program.traits() {
        if let Some(signature) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == symbol)
        {
            return Some(format!("{}::{}", trait_definition.name, signature.name));
        }
    }

    None
}

fn symbol_label(symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!("{symbol:?}")
    } else {
        "<unresolved>".to_owned()
    }
}
