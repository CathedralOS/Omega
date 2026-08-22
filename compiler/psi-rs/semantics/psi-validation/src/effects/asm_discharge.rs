use super::append_effect_call_step;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::statement::StatementNode;

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
                if required_authority
                    == psi_language_core::inline_assembly::AsmAuthorityRequirement::None
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
pub(super) fn validate_asm_intrinsic_declarations(
    program: &TypedTrees,
    operational: &psi_effects::OperationalPlan,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
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
                find_inline_asm_service_path(program, operational, machine.symbol, service_name)
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
    psi_language_core::inline_assembly::AsmAuthorityRequirement,
)> {
    let target = match statement {
        StatementNode::Call(call) => call.target.as_str().to_owned(),
        StatementNode::Assignment(assignment) => {
            let psi_typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(assignment.value)
            else {
                return None;
            };
            call.target.as_str().to_owned()
        }
        _ => return None,
    };

    let function = psi_symbols::BuiltinFunction::asm_intrinsics()
        .into_iter()
        .find(|function| function.name() == target)?;
    let service_name = function.asm_intrinsic_service_name();
    // Label the diagnostic with the SOURCE mnemonic, not the internal name.
    let instruction = match function {
        psi_symbols::BuiltinFunction::AsmHlt => "hlt",
        psi_symbols::BuiltinFunction::AsmPortOut => "out",
        psi_symbols::BuiltinFunction::AsmPortIn => "in",
        psi_symbols::BuiltinFunction::AsmLoadFence => "lfence",
        psi_symbols::BuiltinFunction::AsmStoreFence => "sfence",
        psi_symbols::BuiltinFunction::AsmFullFence => "mfence",
        psi_symbols::BuiltinFunction::AsmDisableInterrupts => "cli",
        psi_symbols::BuiltinFunction::AsmEnableInterrupts => "sti",
        psi_symbols::BuiltinFunction::AsmSnapshotFlags => "pushfq",
        psi_symbols::BuiltinFunction::AsmRestoreFlags => "popfq",
        psi_symbols::BuiltinFunction::AsmReadMsr => "rdmsr",
        psi_symbols::BuiltinFunction::AsmWriteMsr => "wrmsr",
        psi_symbols::BuiltinFunction::AsmReadCr0 => "read_cr0",
        psi_symbols::BuiltinFunction::AsmReadCr2 => "read_cr2",
        psi_symbols::BuiltinFunction::AsmReadCr3 => "read_cr3",
        psi_symbols::BuiltinFunction::AsmReadCr4 => "read_cr4",
        psi_symbols::BuiltinFunction::AsmWriteCr0 => "write_cr0",
        psi_symbols::BuiltinFunction::AsmWriteCr3 => "write_cr3",
        psi_symbols::BuiltinFunction::AsmWriteCr4 => "write_cr4",
        _ => return None,
    };
    let psi_language_core::inline_assembly::AsmCatalogEntry::Contract(contract) =
        psi_language_core::inline_assembly::asm_catalog_entry(instruction)?
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
    operational: &psi_effects::OperationalPlan,
    root_machine_symbol: SymbolHandle,
    service_name: &str,
) -> Option<Vec<InlineAsmServicePathStep>> {
    find_inline_asm_service_path_inner(
        program,
        operational,
        root_machine_symbol,
        service_name,
        &mut Vec::new(),
    )
}

fn find_inline_asm_service_path_inner(
    program: &TypedTrees,
    operational: &psi_effects::OperationalPlan,
    machine_symbol: SymbolHandle,
    service_name: &str,
    visited: &mut Vec<SymbolHandle>,
) -> Option<Vec<InlineAsmServicePathStep>> {
    if visited.contains(&machine_symbol) {
        return None;
    }
    visited.push(machine_symbol);

    let Some(machine) = operational
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        visited.pop();
        return None;
    };
    for state in operational.states.span_or_empty(machine.states) {
        for call in operational.calls.span_or_empty(state.calls) {
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
                    operational,
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
    psi_symbols::BuiltinFunction::asm_intrinsics()
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
