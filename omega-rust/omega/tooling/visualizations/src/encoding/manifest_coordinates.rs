//! Exact declaration and program-point labels shared by checked-program manifests.

use checked_trees::CheckedTrees;
use symbols::SymbolHandle;

pub(crate) fn machine_overload_identity(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
) -> Option<String> {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .and_then(|machine| program.normalized_machine_overload_identity(machine))
        .map(|identity| identity.identity())
}

pub(crate) fn callable_overload_identity(
    program: &CheckedTrees,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> Option<String> {
    if let Some(identity) = machine_overload_identity(program, target_machine) {
        return Some(identity);
    }
    if target_machine == target_state {
        return program.machine_parameter_signature(target_state).map(
            |(declaring_machine, requirement)| {
                program
                    .normalized_machine_parameter_overload_identity(declaring_machine, requirement)
                    .identity()
            },
        );
    }
    program.traits().iter().find_map(|definition| {
        (definition.symbol == target_machine)
            .then(|| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|requirement| requirement.symbol == target_state)
                    .map(|requirement| {
                        program
                            .normalized_trait_requirement_overload_identity(definition, requirement)
                            .identity()
                    })
            })
            .flatten()
    })
}

pub(crate) fn program_point_name(point: facts::ProgramPoint) -> &'static str {
    use facts::ProgramPoint;
    match point {
        ProgramPoint::Global => "global",
        ProgramPoint::Definition { .. } => "definition",
        ProgramPoint::Machine { .. } => "machine",
        ProgramPoint::State { .. } => "state",
        ProgramPoint::Statement { .. } => "statement",
        ProgramPoint::TransitionArm { .. } => "transition_arm",
        ProgramPoint::Call { .. } => "call",
        ProgramPoint::CallRequires { .. } => "call_requires",
        ProgramPoint::CallEnsures { .. } => "call_ensures",
        ProgramPoint::Exit { .. } => "exit",
    }
}

pub(crate) fn exact_program_point_label(
    program: &CheckedTrees,
    point: facts::ProgramPoint,
) -> String {
    use facts::ProgramPoint;

    let symbol = |symbol| qualification_symbol_label(program, symbol);
    match point {
        ProgramPoint::Global => "global".to_owned(),
        ProgramPoint::Definition { symbol: definition } => symbol(definition),
        ProgramPoint::Machine { machine_symbol } => symbol(machine_symbol),
        ProgramPoint::State { state_symbol, .. } => symbol(state_symbol),
        ProgramPoint::Statement {
            state_symbol,
            statement_index,
            ..
        } => format!("{}:statement-{statement_index}", symbol(state_symbol)),
        ProgramPoint::TransitionArm {
            state_symbol,
            statement_index,
            transition_target,
            ..
        } => format!(
            "{}:transition-{statement_index}:target-{}.{}",
            symbol(state_symbol),
            transition_target.arena_index(),
            transition_target.generation()
        ),
        ProgramPoint::Call {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::CallRequires {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-requires-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::CallEnsures {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-ensures-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::Exit {
            state_symbol,
            statement_index,
            transition_target,
            ..
        } => {
            let mut label = format!("{}:exit-{statement_index}", symbol(state_symbol));
            if transition_target.is_valid() {
                label.push_str(&format!(
                    ":target-{}.{}",
                    transition_target.arena_index(),
                    transition_target.generation()
                ));
            }
            label
        }
    }
}

pub(crate) fn state_label_from_symbol(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == symbol)
                .map(|state| format!("{}::{}", machine.name.as_str(), state.name.as_str()))
        })
        .unwrap_or_else(|| symbol_label(program, symbol))
}

pub(crate) fn symbol_label(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!(
            "{} (#{})",
            program.symbols.name(symbol),
            symbol.arena_index()
        )
    } else {
        "invalid".to_owned()
    }
}
pub(crate) fn qualification_symbol_label(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    if !symbol.is_valid() {
        return "<unknown>".to_owned();
    }
    let path = program.symbols.display_path(symbol, "::");
    if path.is_empty() {
        format!("#{}", symbol.arena_index())
    } else {
        path
    }
}
