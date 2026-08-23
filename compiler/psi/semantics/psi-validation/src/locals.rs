use crate::symbols::MachineSymbols;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::statement::StatementNode;

pub(crate) struct WritableRoots<'program, 'state> {
    pub(crate) program: &'program psi_typed_trees::TypedTrees,
    pub(crate) machine_symbols: &'state MachineSymbols<'program>,
    pub(crate) statements: &'state [StatementNode],
    pub(crate) parameters: &'state [StateParameter],
}

impl WritableRoots<'_, '_> {
    /// `bare_reassignment` = the target is the whole local (`x = 2`); member
    /// and index writes pass `false` and keep the ZII fill idiom ungated.
    pub(crate) fn contains_for_write(&self, root_name: &str, bare_reassignment: bool) -> bool {
        self.machine_symbols.has_owned_data(root_name)
            || self.statements.iter().any(|statement| {
                let StatementNode::LocalData(local_data) = statement else {
                    return false;
                };
                if local_data.name.as_str() != root_name {
                    return false;
                }
                if !bare_reassignment {
                    return true;
                }
                local_data.is_mutable || local_is_mutable_reference(self.program, local_data)
            })
            || self
                .parameters
                .iter()
                .any(|parameter| parameter.is_mutable && parameter.name.as_str() == root_name)
    }
}

pub(crate) fn local_is_mutable_reference(
    program: &psi_typed_trees::TypedTrees,
    local_data: &psi_typed_trees::statement::TableLocalData,
) -> bool {
    use psi_typed_trees::types::TypeReferenceNode;
    if !local_data.type_reference.is_valid() {
        return false;
    }
    matches!(
        program
            .type_reference_table
            .type_reference(local_data.type_reference),
        TypeReferenceNode::Reference { access, .. } if access.is_exclusive()
    )
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
