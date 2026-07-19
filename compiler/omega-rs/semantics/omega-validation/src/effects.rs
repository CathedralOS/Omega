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

    validate_pure_discards(program, effect_plan, &mut diagnostics);
    validate_asm_intrinsic_declarations(program, effect_plan, &mut diagnostics);

    for machine_effects in effect_plan.machines() {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_effects.symbol)
        else {
            continue;
        };

        let declared_effects = declared_machine_effect_set(program, machine);
        if declared_effects.is_empty() {
            continue;
        }

        if !declared_effects.contains_all(machine_effects.transitive) {
            let missing = machine_effects.transitive.difference(declared_effects);
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
                if required_authority
                    == omega_core::inline_assembly::AsmAuthorityRequirement::None
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

/// Asm-sourced effects are STRICTLY must-declare -- the empty-declared
/// exemption above does not apply to the privileged tier
/// (privileged_effects_and_binary_trust brief, LOCKED points 2-3: every asm
/// instruction emits its contract, and the function AND every machine that
/// directly-or-indirectly reaches it must declare what it emits; the
/// top-level declared set is what a package manager reads).
///
/// Two rules:
/// 1. A machine whose body CONTAINS an asm intrinsic must declare that
///    instruction's effect (`asm { hlt }` forces `effects machine_control`,
///    `asm { in/out }` forces `effects device_io`).
/// 2. `machine_control` is strict TRANSITIVELY: it originates only from asm,
///    so any machine whose transitive set reaches it must declare it --
///    empty declaration is not an exemption for ring-0 CPU control.
///
/// (device_io transitivity for CALLERS still rides the declared-set ceiling
/// above: it can also originate from boundary-trait rows, whose callers keep
/// the existing empty-declared behavior.)
fn validate_asm_intrinsic_declarations(
    program: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let machine_control = omega_effects::EffectSet::from_name("machine_control")
        .expect("machine_control is a standard effect");

    for machine in program.machines() {
        let declared_effects = declared_machine_effect_set(program, machine);

        // Rule 1: direct emission sites.
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some((instruction, effects, _)) = statement_asm_intrinsic(program, statement)
                else {
                    continue;
                };
                if !declared_effects.contains_all(effects) {
                    let missing = effects.difference(declared_effects);
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` uses asm instruction `{}` but does not declare its \
                         contract: add `effects {}` (every asm instruction's emitted effects \
                         must be declared where they are emitted)",
                        machine.name,
                        instruction,
                        format_effect_set(missing)
                    )));
                }
            }
        }

        // Rule 2: machine_control strict transitivity.
        let Some(machine_effects) = effect_plan
            .machines()
            .iter()
            .find(|entry| entry.symbol == machine.symbol)
        else {
            continue;
        };
        if machine_effects.transitive.contains_all(machine_control)
            && !declared_effects.contains_all(machine_control)
        {
            let mut message = format!(
                "machine `{}` reaches `machine_control` (ring-0 CPU control) but does not \
                 declare it -- machine_control must be declared by every machine that \
                 directly or indirectly reaches it",
                machine.name
            );
            append_effect_paths(
                program,
                effect_plan,
                machine_effects.symbol,
                machine_control,
                &mut message,
            );
            diagnostics.push(Diagnostic::error(message));
        }
    }
}

/// The asm intrinsic a statement carries, as (instruction label, contract
/// effects, required authority): a statement call on an `asm#...` target
/// (`asm { hlt }`, `asm { out .. }`) or an assignment whose value is the
/// `asm#port_in` call (`asm { in dest, port }`). The parser's desugar emits
/// exactly these two shapes and the names are unnameable from source.
fn statement_asm_intrinsic(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Option<(
    &'static str,
    omega_effects::EffectSet,
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
    let effects = omega_effects::asm_intrinsic_effects(function.name())?;
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
        _ => return None,
    };
    let omega_core::inline_assembly::AsmCatalogEntry::Contract(contract) =
        omega_core::inline_assembly::asm_catalog_entry(instruction)?
    else {
        return None;
    };
    Some((instruction, effects, contract.required_authority))
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
                let Some(callee) = resolve_discard_callee(program, effect_plan, call.target_symbol)
                else {
                    continue;
                };

                if callee.effects.is_empty() && !callee.has_mutable_parameter {
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
            return Some(signature_discard_callee(program, signature));
        }
    }

    None
}

/// Boundary-trait signatures have no body to traverse; their declared effect
/// list IS their full effect surface.
fn signature_discard_callee(
    program: &TypedTrees,
    signature: &omega_typed_trees::signature::StateSignature,
) -> DiscardCallee {
    let mut effects = omega_effects::EffectSet::empty();
    for effect in program.state_signature_effects(signature) {
        effects.insert_name(effect.as_str());
    }

    DiscardCallee {
        effects,
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
    effects
}

fn format_effect_set(effects: omega_effects::EffectSet) -> String {
    if effects.is_empty() {
        return "<none>".to_owned();
    }

    effects.names().collect::<Vec<_>>().join(", ")
}
