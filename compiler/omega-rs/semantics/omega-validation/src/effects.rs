use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::statement::StatementNode;

pub fn validate_behavior_plan(
    program: &TypedTrees,
    operations: &omega_effects::OperationalPlan,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let service_reaches = omega_effects::infer_service_reaches(program, operations);

    validate_pure_discards(program, operations, &service_reaches, &mut diagnostics);
    validate_asm_intrinsic_declarations(program, operations, &service_reaches, &mut diagnostics);
    validate_service_reach_ceilings(program, &service_reaches, &mut diagnostics);

    for machine_operations in operations.machines() {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_operations.symbol)
        else {
            continue;
        };

        // Requirements, boundaries, accepted contracts, and external
        // realizations publish closed operational ceilings. Omission is an
        // authored `false`, unlike omission on a private checked body, where
        // both axes are inferred. Validate the declaration-free body fixed
        // points so a published machine cannot launder behavior through a
        // local helper or a pinned requirement.
        if machine.supply_mode != omega_core::semantics::MachineSupplyMode::CheckedBody {
            if machine_operations.body_may_suspend && !machine.suspends {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` omits `suspends;` from its published contract, but its body may suspend",
                    machine.name
                )));
            }
            if machine_operations.body_may_block && !machine.blocks {
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
    service_reaches: &omega_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        let publishes = machine.supply_mode
            != omega_core::semantics::MachineSupplyMode::CheckedBody
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
    services: &[omega_core::semantics::ServiceReachId],
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

/// The v0 `machine_control`/port-I/O DISCHARGE gate
/// (privileged_effects_and_binary_trust brief, LOCKED point 4 + the M3
/// actionable subset): declaring an effect labels the code truthfully;
/// DISCHARGE proves the code is PERMITTED to run it. Until the
/// owns-the-machine capability token lands (the parked capability_lifecycle
/// arc), discharge v0 is "permitted in the freestanding boundary root": the
/// boot root trivially owns the machine, and a HOSTED build has no business
/// executing `hlt` (ring-3 faults) or raw port I/O (unmapped I/O bitmap
/// faults) -- so authority-bearing asm intrinsics in a non-freestanding build
/// are a compile error, not a runtime fault. Effect-free instructions such as
/// x86 memory fences do not require machine ownership.
pub fn validate_asm_discharge(
    program: &TypedTrees,
    freestanding: bool,
) -> Result<(), Vec<Diagnostic>> {
    if freestanding {
        return Ok(());
    }

    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some((instruction, _, required_authority)) =
                    statement_asm_intrinsic(program, statement)
                else {
                    continue;
                };
                if required_authority == omega_core::inline_assembly::AsmAuthorityRequirement::None
                {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` uses asm instruction `{}`, which requires a FREESTANDING \
                     boundary root (v0 discharge: only code that owns the machine may emit \
                     privileged instructions; a hosted build would fault at ring 3). Set \
                     `b.freestanding = true` in build.omg, or remove the asm block",
                    machine.name, instruction
                )));
            }
        }
    }

    crate::finish_diagnostics(diagnostics)
}

/// Asm-sourced service reach is STRICTLY must-declare -- private inference does
/// not exempt the privileged tier
/// (privileged_effects_and_binary_trust brief, LOCKED points 2-3: every asm
/// instruction emits its contract, and the function AND every machine that
/// directly-or-indirectly reaches it must publish the service; the top-level
/// row is what a package manager reads).
///
/// Two rules:
/// 1. A machine whose body CONTAINS an asm intrinsic must declare that
///    canonical service (`asm { hlt }` forces `reaches MachineControl`,
///    `asm { in/out }` forces `reaches PortIo`).
/// 2. Both asm services are strict TRANSITIVELY along known checked call
///    paths. This provenance-specific rule does not turn an unrelated
///    boundary call that happens to reach the same service into inline asm.
fn validate_asm_intrinsic_declarations(
    program: &TypedTrees,
    operations: &omega_effects::OperationalPlan,
    service_reaches: &omega_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        let declared_services = program
            .service_reach_rows
            .services(machine.service_reach_row);
        let mut direct_asm_services = Vec::new();

        // Rule 1: direct emission sites.
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some((instruction, service_name, _)) =
                    statement_asm_intrinsic(program, statement)
                else {
                    continue;
                };
                let Some(service_name) = service_name else {
                    continue;
                };
                if !direct_asm_services.contains(&service_name) {
                    direct_asm_services.push(service_name);
                }
                let Some(service) = program.service_reaches.id_for_name(service_name) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` uses asm instruction `{}`, which reaches canonical \
                         service `{service_name}`, but that service identity is unavailable: \
                         add `use omega::language::core::assembly;`",
                        machine.name, instruction
                    )));
                    continue;
                };
                if !declared_services.contains(&service) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` uses asm instruction `{}` but does not declare its \
                         service contract: add `reaches {service_name}` (every asm \
                         instruction's service reach must be declared where it is emitted)",
                        machine.name, instruction
                    )));
                }
            }
        }

        // The normalized summary is also an invariant check: a resolved asm
        // service must appear in the ordinary canonical reach fixed point.
        let Some(machine_services) = service_reaches.for_machine(machine.symbol) else {
            continue;
        };
        for service_name in ["MachineControl", "PortIo"] {
            let Some(service) = program.service_reaches.id_for_name(service_name) else {
                continue;
            };
            if direct_asm_services.contains(&service_name) {
                debug_assert!(
                    service_reaches
                        .services(machine_services.inferred_transitive)
                        .contains(&service)
                );
                continue;
            }
            let Some(path) =
                find_inline_asm_service_path(program, operations, machine.symbol, service_name)
            else {
                continue;
            };
            debug_assert!(
                service_reaches
                    .services(machine_services.inferred_transitive)
                    .contains(&service)
            );
            if declared_services.contains(&service) {
                continue;
            }
            let mut message = format!(
                "machine `{}` reaches inline-assembly service `{service_name}` but does not \
                 declare it -- `{service_name}` must be declared by every machine that \
                 directly or indirectly reaches the asm instruction",
                machine.name
            );
            append_inline_asm_service_path(program, service_name, &path, &mut message);
            diagnostics.push(Diagnostic::error(message));
        }
    }
}

/// The asm intrinsic a statement carries, as (instruction label, contract
/// service identity, required authority): a statement call on an `asm#...` target
/// (`asm { hlt }`, `asm { out .. }`) or an assignment whose value is the
/// `asm#port_in` call (`asm { in dest, port }`). The parser's desugar emits
/// exactly these two shapes and the names are unnameable from source.
fn statement_asm_intrinsic(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Option<(
    &'static str,
    Option<&'static str>,
    omega_core::inline_assembly::AsmAuthorityRequirement,
)> {
    let target = match statement {
        StatementNode::Call(call) => call.target.as_str().to_owned(),
        StatementNode::Assignment(assignment) => {
            let omega_typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(assignment.value)
            else {
                return None;
            };
            call.target.as_str().to_owned()
        }
        _ => return None,
    };

    let function = omega_core::symbols::BuiltinFunction::asm_intrinsics()
        .into_iter()
        .find(|function| function.name() == target)?;
    let service_name = function.asm_intrinsic_service_name();
    // Label the diagnostic with the SOURCE mnemonic, not the internal name.
    let instruction = match function {
        omega_core::symbols::BuiltinFunction::AsmHlt => "hlt",
        omega_core::symbols::BuiltinFunction::AsmPortOut => "out",
        omega_core::symbols::BuiltinFunction::AsmPortIn => "in",
        omega_core::symbols::BuiltinFunction::AsmLoadFence => "lfence",
        omega_core::symbols::BuiltinFunction::AsmStoreFence => "sfence",
        omega_core::symbols::BuiltinFunction::AsmFullFence => "mfence",
        omega_core::symbols::BuiltinFunction::AsmDisableInterrupts => "cli",
        omega_core::symbols::BuiltinFunction::AsmEnableInterrupts => "sti",
        omega_core::symbols::BuiltinFunction::AsmSnapshotFlags => "pushfq",
        omega_core::symbols::BuiltinFunction::AsmRestoreFlags => "popfq",
        omega_core::symbols::BuiltinFunction::AsmReadMsr => "rdmsr",
        omega_core::symbols::BuiltinFunction::AsmWriteMsr => "wrmsr",
        omega_core::symbols::BuiltinFunction::AsmReadCr0 => "read_cr0",
        omega_core::symbols::BuiltinFunction::AsmReadCr2 => "read_cr2",
        omega_core::symbols::BuiltinFunction::AsmReadCr3 => "read_cr3",
        omega_core::symbols::BuiltinFunction::AsmReadCr4 => "read_cr4",
        omega_core::symbols::BuiltinFunction::AsmWriteCr0 => "write_cr0",
        omega_core::symbols::BuiltinFunction::AsmWriteCr3 => "write_cr3",
        omega_core::symbols::BuiltinFunction::AsmWriteCr4 => "write_cr4",
        _ => return None,
    };
    let omega_core::inline_assembly::AsmCatalogEntry::Contract(contract) =
        omega_core::inline_assembly::asm_catalog_entry(instruction)?
    else {
        return None;
    };
    Some((instruction, service_name, contract.required_authority))
}

#[derive(Debug, Clone, Copy)]
struct InlineAsmServicePathStep {
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    target_machine_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
}

fn find_inline_asm_service_path(
    program: &TypedTrees,
    operations: &omega_effects::OperationalPlan,
    root_machine_symbol: SymbolHandle,
    service_name: &str,
) -> Option<Vec<InlineAsmServicePathStep>> {
    find_inline_asm_service_path_inner(
        program,
        operations,
        root_machine_symbol,
        service_name,
        &mut Vec::new(),
    )
}

fn find_inline_asm_service_path_inner(
    program: &TypedTrees,
    operations: &omega_effects::OperationalPlan,
    machine_symbol: SymbolHandle,
    service_name: &str,
    visited: &mut Vec<SymbolHandle>,
) -> Option<Vec<InlineAsmServicePathStep>> {
    if visited.contains(&machine_symbol) {
        return None;
    }
    visited.push(machine_symbol);

    let Some(machine) = operations
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        visited.pop();
        return None;
    };
    for state in operations.states.span_or_empty(machine.states) {
        for call in operations.calls.span_or_empty(state.calls) {
            let step = InlineAsmServicePathStep {
                caller_machine_symbol: machine_symbol,
                caller_state_symbol: state.symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_machine_symbol: call.target_machine_symbol,
                target_state_symbol: call.target_state_symbol,
            };
            if asm_service_name_for_symbol(program, call.target_state_symbol) == Some(service_name)
            {
                visited.pop();
                return Some(vec![step]);
            }
            if call.target_machine_symbol.is_valid()
                && let Some(mut suffix) = find_inline_asm_service_path_inner(
                    program,
                    operations,
                    call.target_machine_symbol,
                    service_name,
                    visited,
                )
            {
                let mut path = vec![step];
                path.append(&mut suffix);
                visited.pop();
                return Some(path);
            }
        }
    }
    visited.pop();
    None
}

fn asm_service_name_for_symbol(program: &TypedTrees, symbol: SymbolHandle) -> Option<&'static str> {
    omega_core::symbols::BuiltinFunction::asm_intrinsics()
        .into_iter()
        .find(|function| program.symbols.builtin_function_symbol(*function) == Some(symbol))?
        .asm_intrinsic_service_name()
}

fn append_inline_asm_service_path(
    program: &TypedTrees,
    service_name: &str,
    path: &[InlineAsmServicePathStep],
    message: &mut String,
) {
    message.push_str("\n\ncall path to inline assembly for `");
    message.push_str(service_name);
    message.push_str("`:");
    for (index, step) in path.iter().enumerate() {
        append_effect_call_step(
            program,
            message,
            index + 1,
            step.caller_machine_symbol,
            step.caller_state_symbol,
            step.statement_index,
            step.call_ordinal,
            step.target_machine_symbol,
            step.target_state_symbol,
        );
    }
}

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
    operations: &omega_effects::OperationalPlan,
    service_reaches: &omega_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let classification = omega_typed_trees::proof_only::classify(program);
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
                    operations,
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
) -> Option<&omega_typed_trees::machine::Machine> {
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
    operations: &omega_effects::OperationalPlan,
    service_reaches: &omega_effects::ServiceReachInferencePlan,
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
        let operational = operations
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
            may_suspend: operational.0,
            may_block: operational.1,
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
    trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
    signature: &omega_typed_trees::signature::StateSignature,
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

fn append_effect_call_step(
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
) -> Option<&omega_typed_trees::state::State> {
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
