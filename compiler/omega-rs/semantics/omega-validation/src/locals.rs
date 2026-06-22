use crate::symbols::MachineSymbols;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::signature::StateParameter;
use omega_typed_trees::statement::StatementNode;

pub(crate) struct WritableRoots<'program, 'state> {
    pub(crate) machine_symbols: &'state MachineSymbols<'program>,
    pub(crate) statements: &'state [StatementNode],
    pub(crate) parameters: &'state [StateParameter],
}

impl WritableRoots<'_, '_> {
    pub(crate) fn contains(&self, root_name: &str) -> bool {
        self.machine_symbols.has_owned_data(root_name)
            || self.statements.iter().any(|statement| {
                let StatementNode::LocalData(local_data) = statement else {
                    return false;
                };

                local_data.name.as_str() == root_name
            })
            || self
                .parameters
                .iter()
                .any(|parameter| parameter.is_mutable && parameter.name.as_str() == root_name)
    }
}

pub(crate) fn validate_local_data_names(
    statements: &[StatementNode],
    machine_symbols: &MachineSymbols<'_>,
    parameters: &[StateParameter],
    machine_name: &str,
    state_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (statement_index, statement) in statements.iter().enumerate() {
        let StatementNode::LocalData(local_data) = statement else {
            continue;
        };

        if machine_symbols.has_member(local_data.name.as_str())
            || parameters
                .iter()
                .any(|parameter| parameter.name == local_data.name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{machine_name}` state `{state_name}` local data `{}` conflicts with an existing name",
                local_data.name
            )));
            continue;
        }

        if statements[..statement_index].iter().any(|previous| {
            matches!(
                previous,
                StatementNode::LocalData(previous) if previous.name == local_data.name
            )
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{machine_name}` state `{state_name}` has duplicate local data `{}`",
                local_data.name
            )));
        }
    }
}
