use super::{StateMutationKind, StateMutationLowering};
use crate::control_flow::StateKey;
use crate::state_analysis::StateAnalysisContext;
use omega_typed_program::Program;
use omega_typed_program::expression::Expression;
use omega_typed_program::statement::Statement;

pub(super) fn mutation_lowering(
    context: &StateAnalysisContext,
    source_key: StateKey,
    statement_index: usize,
    mutation_kind: StateMutationKind,
) -> StateMutationLowering {
    if context.state_mutation_is_already_lowered_by_key(source_key, statement_index) {
        return StateMutationLowering::AlreadyLowered;
    }

    match mutation_kind {
        StateMutationKind::Local => StateMutationLowering::NeedsLocalWrite,
        StateMutationKind::MachineOwned => StateMutationLowering::NeedsMachineOwnedWrite,
        StateMutationKind::ParameterOrAlias => StateMutationLowering::NeedsAliasWrite,
        StateMutationKind::Unknown => StateMutationLowering::Unknown,
    }
}

pub(super) fn mutation_kind(
    program: &Program,
    machine_name: &str,
    state: &omega_typed_program::state::State,
    target: &Expression,
) -> StateMutationKind {
    let Some(root_name) = root_place_name(target) else {
        return StateMutationKind::Unknown;
    };
    let Some(machine) = program
        .machines
        .iter()
        .find(|machine| machine.name == machine_name)
    else {
        return StateMutationKind::Unknown;
    };

    if machine
        .owned_data
        .iter()
        .any(|owned_data| owned_data.name == root_name)
    {
        return StateMutationKind::MachineOwned;
    }

    if state
        .parameters
        .iter()
        .any(|parameter| parameter.name == root_name)
    {
        return StateMutationKind::ParameterOrAlias;
    }

    if state.statements.iter().any(|statement| {
        matches!(statement, Statement::LocalData(local_data) if local_data.name == root_name)
    }) {
        return StateMutationKind::Local;
    }

    StateMutationKind::Unknown
}

fn root_place_name(expression: &Expression) -> Option<&str> {
    match expression {
        Expression::Name(path) => path.first().map(|name| name.as_str()),
        Expression::Indexed(indexed) => root_place_name(&indexed.collection),
        Expression::Mutable(expression) => root_place_name(expression),
        _ => None,
    }
}
