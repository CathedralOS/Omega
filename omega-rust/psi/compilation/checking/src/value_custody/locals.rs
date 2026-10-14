use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use diagnostics::Diagnostic;
use language_core::is_self_receiver;
use typed_trees::signature::StateParameter;
use typed_trees::statement::StatementNode;
use typed_trees::{machine::Machine, state::State};
mod type_value_scope;
mod value_scope;
pub(crate) use value_scope::StateValueScope;

/// An executable binding belongs to this state and is identified by its
/// retained declaration, not another state's spelling.
/// Alpha-rebased contracts retain their authored diagnostic words; only the
/// exact declaration owner and current parameter/prior-local roster establish
/// scope. Requiring those words to be renamed would undo resolved identity.
pub(crate) fn state_binding_type(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    prior_statements: &[StatementNode],
    symbol: symbols::SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }
    let declaration = program.symbols.get(symbol);
    if declaration.parent != state.symbol {
        return None;
    }
    match declaration.kind {
        symbols::SymbolKind::Parameter => program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol)
            .map(|parameter| parameter.type_reference),
        symbols::SymbolKind::Local => {
            prior_statements
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.symbol == symbol => {
                        Some(local.type_reference)
                    }
                    _ => None,
                })
        }
        _ => None,
    }
}

/// A value root must name current-state storage or a non-storage declaration.
pub(crate) fn state_value_root_is_known(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    prior_statements: &[StatementNode],
    symbols: &TopLevelSymbols<'_>,
    root: symbols::SymbolHandle,
    name: &str,
) -> bool {
    let kind = program.symbols.get(root).kind;
    if is_self_receiver(name)
        || matches!(
            kind,
            symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
        )
    {
        return crate::value_custody::locals::state_binding_type(
            program,
            state,
            prior_statements,
            root,
        )
        .is_some();
    }
    if root.is_valid() && kind == symbols::SymbolKind::Unknown {
        return false;
    }
    // Generated case operands retain the selected declaration even when their
    // diagnostic carrier includes a package prefix absent from the roster name.
    // Storage roots above must still belong to this exact state.
    // A named implementation or evidence binder is a declaration namespace
    // that qualifies a requirement call; it holds no runtime value.
    if root.is_valid()
        && matches!(
            kind,
            symbols::SymbolKind::Conformance | symbols::SymbolKind::ConformanceParameter
        )
    {
        return true;
    }
    if symbols.has_type_symbol(root)
        || symbols.has_type(name)
        || symbols.machine(name).is_some()
        || symbols.trait_definition(name).is_some()
    {
        return true;
    }
    // A generic attached method may use its container's const parameter as a
    // value in the authored template. Instance desugaring replaces this name
    // with the concrete integer before the executable clone is validated.
    let (scope, _) =
        program.machine_type_reference_scope(machine, |left, right| left.name == right.name);
    if scope.iter().any(|parameter| {
        parameter.name.as_str() == name
            && parameter.kind.is_const_or_value()
            && (!root.is_valid() || parameter.symbol == root)
    }) {
        return true;
    }
    if let Some(attached_data) = &machine.attached_data
        && program.data_definitions().iter().any(|definition| {
            definition.name == *attached_data
                && program
                    .data_type_parameters(definition)
                    .iter()
                    .any(|parameter| {
                        parameter.name.as_str() == name && parameter.kind.is_const_or_value()
                    })
        })
    {
        return true;
    }
    // Enum case constant used bare (`let s: Signal = Red`).
    for definition in program.data_definitions() {
        for member in program.data_members(definition) {
            if let Some(variant) = member.as_variant()
                && variant.name.as_str() == name
            {
                return true;
            }
        }
    }
    crate::value_custody::locals::state_binding_type(program, state, prior_statements, root)
        .is_some()
}

pub(crate) struct WritableRoots<'program, 'state> {
    pub(crate) program: &'program dyn crate::CheckingInput,
    pub(crate) machine: &'program Machine,
    pub(crate) machine_symbols: &'state MachineSymbols<'program>,
    pub(crate) statements: &'state [StatementNode],
    pub(crate) parameters: &'state [StateParameter],
}

impl WritableRoots<'_, '_> {
    /// `bare_reassignment` = the target is the whole local (`x = 2`); member
    /// and index writes pass `false` and keep the ZII fill idiom ungated.
    pub(crate) fn contains_for_write(&self, root_name: &str, bare_reassignment: bool) -> bool {
        (self.machine_symbols.has_owned_data(root_name)
            && self
                .program
                .machine_owned_data(self.machine)
                .iter()
                .any(|owned| owned.name.as_str() == root_name))
            || self.statements.iter().any(|statement| {
                let Some(local_data) = statement.as_local_data() else {
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

    /// Whether a root is reachable at all from this state, with no
    /// mutability demand: an attached field behind any `self`, machine-owned
    /// data, a declared local, or a parameter of either polarity. This is the
    /// demand of an operation whose sealed requirement takes a shared
    /// receiver (the atomic families); it still refuses an undeclared root.
    pub(crate) fn contains_for_shared_access(&self, root_name: &str) -> bool {
        (self.machine_symbols.has_owned_data(root_name)
            && (self
                .program
                .machine_owned_data(self.machine)
                .iter()
                .any(|owned| owned.name.as_str() == root_name)))
            || self.statements.iter().any(|statement| {
                matches!(
                    statement,
                    StatementNode::LocalData(local_data) if local_data.name.as_str() == root_name
                )
            })
            || self
                .parameters
                .iter()
                .any(|parameter| parameter.name.as_str() == root_name)
    }
}

pub(crate) fn local_is_mutable_reference(
    program: &dyn crate::CheckingInput,
    local_data: &typed_trees::statement::TableLocalData,
) -> bool {
    use typed_trees::types::TypeReferenceNode;
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
        let Some(local_data) = statement.as_local_data() else {
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
