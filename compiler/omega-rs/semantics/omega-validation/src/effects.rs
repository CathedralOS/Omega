use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::statement::StatementNode;

pub fn validate_effect_plan(
    program: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let service_reaches = omega_effects::infer_service_reaches(program, effect_plan);

    validate_pure_discards(program, effect_plan, &service_reaches, &mut diagnostics);
    validate_asm_intrinsic_declarations(program, effect_plan, &service_reaches, &mut diagnostics);

    for machine_effects in effect_plan.machines() {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_effects.symbol)
        else {
            continue;
        };

        let declared_effects = declared_machine_effect_set(program, machine);
        let reached_effects = machine_effects.transitive;
        if !declared_effects.is_empty() && !declared_effects.contains_all(reached_effects) {
            let missing = reached_effects.difference(declared_effects);
            let mut message = format!(
                "machine `{}` declares effects `{}` but reaches undeclared effects `{}`",
                machine.name,
                format_effect_set(declared_effects),
                format_effect_set(missing)
            );
            append_effect_paths(
                program,
                effect_plan,
                machine_effects.symbol,
                missing,
                &mut message,
            );
            diagnostics.push(Diagnostic::error(message));
        }

        // Requirements, boundaries, accepted contracts, and external
        // realizations publish closed operational ceilings. Omission is an
        // authored `false`, unlike omission on a private checked body, where
        // both axes are inferred. Validate the declaration-free body fixed
        // points so a published machine cannot launder behavior through a
        // local helper or a pinned requirement.
        if machine.supply_mode != omega_core::semantics::MachineSupplyMode::CheckedBody {
            if machine_effects.body_may_suspend && !machine.suspends {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` omits `suspends;` from its published contract, but its body may suspend",
                    machine.name
                )));
            }
            if machine_effects.body_may_block && !machine.blocks {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` omits `blocks;` from its published contract, but its body may block",
                    machine.name
                )));
            }
        }
    }

    crate::finish_diagnostics(diagnostics)
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
///    canonical service (`asm { hlt }` forces `effects MachineControl`,
///    `asm { in/out }` forces `effects PortIo`).
/// 2. Both asm services are strict TRANSITIVELY along known checked call
///    paths. This provenance-specific rule does not turn an unrelated
///    boundary call that happens to reach the same service into inline asm.
fn validate_asm_intrinsic_declarations(
    program: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
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
                         service contract: add `effects {service_name}` (every asm \
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
                find_inline_asm_service_path(program, effect_plan, machine.symbol, service_name)
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
    effect_plan: &omega_effects::EffectPlan,
    root_machine_symbol: SymbolHandle,
    service_name: &str,
) -> Option<Vec<InlineAsmServicePathStep>> {
    find_inline_asm_service_path_inner(
        program,
        effect_plan,
        root_machine_symbol,
        service_name,
        &mut Vec::new(),
    )
}

fn find_inline_asm_service_path_inner(
    program: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
    machine_symbol: SymbolHandle,
    service_name: &str,
    visited: &mut Vec<SymbolHandle>,
) -> Option<Vec<InlineAsmServicePathStep>> {
    if visited.contains(&machine_symbol) {
        return None;
    }
    visited.push(machine_symbol);

    let Some(machine) = effect_plan
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        visited.pop();
        return None;
    };
    for state in effect_plan.states.span_or_empty(machine.states) {
        for call in effect_plan.calls.span_or_empty(state.calls) {
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
                    effect_plan,
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
/// empty effect set AND no `&mut`/out parameters -- the statement is dead
/// runtime code. AMENDED (owner, 2026-07-12) from reject to WARN, for
/// uniform compilation: the statement always compiles, and the deadness
/// surfaces as a warning ONLY outside proof use. Proof use is silent by
/// design -- a statement call whose callee is a PROOF MACHINE is a CITATION
/// (ch10 "Citing Proofs": invoked for its ensures, erased at codegen), and
/// inside a proof machine's own body every discard is compile-time
/// material. The effect surface is read from the inferred plan's
/// TRANSITIVE machine set, not the declared signature surface: a machine that
/// declares nothing but transitively calls `console.write` is effectful, and
/// the declared-vs-reached ceiling above only fires when something IS declared,
/// so the declared set alone would produce false "pure" verdicts.
fn validate_pure_discards(
    program: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
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
                    effect_plan,
                    service_reaches,
                    call.target_symbol,
                ) else {
                    continue;
                };

                if callee.effects.is_empty()
                    && !callee.has_service_reach
                    && !callee.has_mutable_parameter
                {
                    diagnostics.push(Diagnostic::warning(format!(
                        "`_ = {target}(...);` discards a provably pure call: `{target}` has no effects and no mutable out-parameters, so the discard is dead code; remove the statement or use the result",
                        target = call.target.as_str()
                    )));
                }
            }
        }
    }
}

struct DiscardCallee {
    effects: omega_effects::EffectSet,
    has_service_reach: bool,
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
    effect_plan: &omega_effects::EffectPlan,
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
        let effects = effect_plan
            .machines()
            .iter()
            .find(|entry| entry.symbol == machine.symbol)
            .map(|entry| entry.transitive)?;

        return Some(DiscardCallee {
            effects,
            has_service_reach: service_reaches
                .for_machine(machine.symbol)
                .is_some_and(|summary| !service_reaches.services(summary.effective).is_empty()),
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
/// observable even when the lowercase compatibility list is empty.
fn signature_discard_callee(
    program: &TypedTrees,
    trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
    signature: &omega_typed_trees::signature::StateSignature,
) -> DiscardCallee {
    let mut effects = omega_effects::EffectSet::empty();
    for effect in program.state_signature_effects(signature) {
        effects.insert_name(effect.as_str());
    }

    DiscardCallee {
        effects,
        has_service_reach: trait_definition.is_boundary
            || !program
                .service_reach_rows
                .services(signature.service_reach_row)
                .is_empty(),
        has_mutable_parameter: program
            .state_signature_parameters(signature)
            .iter()
            .any(|parameter| parameter.is_mutable),
    }
}

fn append_effect_paths(
    program: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
    root_machine_symbol: SymbolHandle,
    missing: omega_effects::EffectSet,
    message: &mut String,
) {
    for effect_name in missing.names() {
        let Some(effect) = omega_effects::EffectSet::from_name(effect_name) else {
            continue;
        };
        let Some(path) = effect_plan.find_effect_path(root_machine_symbol, effect) else {
            continue;
        };

        message.push_str("\n\ncall path for `");
        message.push_str(effect_name);
        message.push_str("`:");
        append_effect_path_source(program, &path.source, message, 1);
    }
}

fn append_effect_path_source(
    program: &TypedTrees,
    source: &omega_effects::EffectPathSource,
    message: &mut String,
    depth: usize,
) {
    match source {
        omega_effects::EffectPathSource::MachineDirect { machine_symbol } => {
            message.push('\n');
            push_indent(message, depth);
            message.push_str("source: machine `");
            message.push_str(&machine_label(program, *machine_symbol));
            message.push_str("` directly declares the effect");
        }
        omega_effects::EffectPathSource::StateDirect {
            machine_symbol,
            state_symbol,
        } => {
            message.push('\n');
            push_indent(message, depth);
            message.push_str("source: state `");
            message.push_str(&callable_state_label(
                program,
                *machine_symbol,
                *state_symbol,
            ));
            message.push_str("` directly declares the effect");
        }
        omega_effects::EffectPathSource::CallDirect {
            caller_machine_symbol,
            caller_state_symbol,
            statement_index,
            call_ordinal,
            target_machine_symbol,
            target_state_symbol,
        } => {
            append_effect_call_step(
                program,
                message,
                depth,
                *caller_machine_symbol,
                *caller_state_symbol,
                *statement_index,
                *call_ordinal,
                *target_machine_symbol,
                *target_state_symbol,
            );
            message.push('\n');
            push_indent(message, depth + 1);
            message.push_str("source: target signature declares the effect");
        }
        omega_effects::EffectPathSource::ThroughCall {
            caller_machine_symbol,
            caller_state_symbol,
            statement_index,
            call_ordinal,
            target_machine_symbol,
            target_state_symbol,
            target_source,
        } => {
            append_effect_call_step(
                program,
                message,
                depth,
                *caller_machine_symbol,
                *caller_state_symbol,
                *statement_index,
                *call_ordinal,
                *target_machine_symbol,
                *target_state_symbol,
            );
            append_effect_path_source(program, target_source, message, depth + 1);
        }
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

fn declared_machine_effect_set(
    program: &TypedTrees,
    machine: &Machine,
) -> omega_effects::EffectSet {
    let mut effects = omega_effects::EffectSet::empty();
    for effect in program.machine_effects(machine) {
        effects.insert_name(effect.as_str());
    }
    // Compatibility only: the generic legacy ceiling and flow graph still
    // carry lowercase bits. Derive their conservative projection from
    // canonical, symbol-resolved rows so authoring never regresses to those
    // names.
    for service in program
        .service_reach_rows
        .services(machine.service_reach_row)
    {
        let Some(definition) = program.service_reaches.definition(*service) else {
            continue;
        };
        match definition.name.as_str() {
            "MachineControl" => {
                effects.insert_name("machine_control");
            }
            "PortIo" => {
                effects.insert_name("device_io");
            }
            // Boundary-service identities have no one-to-one member in the
            // retired global catalog. Its conservative host-facing bit is the
            // compatibility projection until the generic ceiling/graph path
            // consumes normalized rows directly.
            _ => {
                effects.insert_name("host_boundary");
            }
        }
    }
    effects
}

fn format_effect_set(effects: omega_effects::EffectSet) -> String {
    if effects.is_empty() {
        return "<none>".to_owned();
    }

    effects.names().collect::<Vec<_>>().join(", ")
}
