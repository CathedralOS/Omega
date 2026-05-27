use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::machine::Machine;

pub fn validate_effect_plan(
    program: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

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

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
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
    for platform in program.platforms() {
        if let Some(signature) = program
            .platform_state_signatures(platform)
            .iter()
            .find(|signature| signature.symbol == symbol)
        {
            return Some(signature.name.to_string());
        }
    }

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
