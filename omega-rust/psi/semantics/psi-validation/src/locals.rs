use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::data::{DataMember, TypeParameterKind};
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::statement::StatementNode;
use psi_typed_trees::{TypedTrees, machine::Machine, state::State};

/// A named operator's complete declaration namespace is not a storage read.
/// Retained value identities never acquire namespace status from spelling.
pub(crate) fn is_named_operator_namespace(
    program: &TypedTrees,
    receiver_symbols: &[psi_symbols::SymbolHandle],
    namespace: &[&str],
    target_symbol: psi_symbols::SymbolHandle,
    target: &str,
    argument_count: usize,
) -> bool {
    if namespace.is_empty()
        || receiver_symbols.iter().any(|symbol| {
            symbol.is_valid()
                && !matches!(
                    program.symbols.get(*symbol).kind,
                    psi_symbols::SymbolKind::Module
                        | psi_symbols::SymbolKind::BuiltinType
                        | psi_symbols::SymbolKind::Data
                        | psi_symbols::SymbolKind::Domain
                        | psi_symbols::SymbolKind::Machine
                        | psi_symbols::SymbolKind::Trait
                )
        })
    {
        return false;
    }
    psi_typed_trees::operator::resolve_named_call(
        program,
        psi_symbols::SymbolHandle::invalid(),
        Some(namespace),
        target,
        argument_count,
        false,
    )
    .is_some_and(|operator| !target_symbol.is_valid() || target_symbol == operator.symbol)
}

pub(crate) fn expression_call_has_operator_namespace(
    program: &TypedTrees,
    call: &psi_typed_trees::expression::TableCallExpression,
) -> bool {
    let table = &program.expression_table;
    let psi_typed_trees::expression::ExpressionNode::Name(path) = table.expression(call.receiver)
    else {
        return false;
    };
    let receiver_symbols: Vec<_> = [path.head_symbol, path.symbol]
        .into_iter()
        .chain(
            table
                .name_path_member_symbols(path.member_symbols)
                .iter()
                .copied(),
        )
        .collect();
    let namespace: Vec<_> = table
        .name_path_members(path.members)
        .iter()
        .map(|member| member.as_str())
        .collect();
    is_named_operator_namespace(
        program,
        &receiver_symbols,
        &namespace,
        call.target_symbol,
        call.target.as_str(),
        table.expression_handles(call.arguments).len(),
    )
}

/// An executable binding belongs to this state and is identified by its
/// retained declaration, not another state's spelling. `self` may use the
/// machine symbol retained by typed expression lowering.
pub(crate) fn state_binding_type(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    prior_statements: &[StatementNode],
    symbol: psi_symbols::SymbolHandle,
    name: &str,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }
    if name == "self" && symbol == machine.symbol {
        return program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .map(|parameter| parameter.type_reference);
    }
    let declaration = program.symbols.get(symbol);
    if declaration.parent != state.symbol || program.symbols.name(symbol) != name {
        return None;
    }
    match declaration.kind {
        psi_symbols::SymbolKind::Parameter => program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol && parameter.name.as_str() == name)
            .map(|parameter| parameter.type_reference),
        psi_symbols::SymbolKind::Local => {
            prior_statements
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local)
                        if local.symbol == symbol && local.name.as_str() == name =>
                    {
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
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    prior_statements: &[StatementNode],
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    root: psi_symbols::SymbolHandle,
    name: &str,
) -> bool {
    let kind = program.symbols.get(root).kind;
    if name == "self"
        || matches!(
            kind,
            psi_symbols::SymbolKind::Local | psi_symbols::SymbolKind::Parameter
        )
    {
        return crate::locals::state_binding_type(
            program,
            machine,
            state,
            prior_statements,
            root,
            name,
        )
        .is_some();
    }
    if root.is_valid() && kind == psi_symbols::SymbolKind::Unknown {
        return false;
    }
    // Field of the receiver data (bare `fld` == `self.fld`), owned data, or a
    // callable field.
    if program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.is_self)
        && (machine_symbols.has_member(name)
            || machine_symbols.has_owned_data(name)
            || machine_symbols.callable_field_type(name).is_some())
    {
        return true;
    }
    // Top-level symbol: a type, machine, or trait spelled bare.
    if symbols.has_type(name)
        || symbols.machine(name).is_some()
        || symbols.trait_definition(name).is_some()
    {
        return true;
    }
    // A generic attached method may use its container's const parameter as a
    // value in the authored template. Instance desugaring replaces this name
    // with the concrete integer before the executable clone is validated.
    if program
        .machine_type_parameters(machine)
        .iter()
        .any(|parameter| {
            parameter.name.as_str() == name
                && matches!(&parameter.kind, TypeParameterKind::Const { .. })
        })
    {
        return true;
    }
    if let Some(attached_data) = &machine.attached_data
        && program.data_definitions().iter().any(|definition| {
            definition.name == *attached_data
                && program
                    .data_type_parameters(definition)
                    .iter()
                    .any(|parameter| {
                        parameter.name.as_str() == name
                            && matches!(&parameter.kind, TypeParameterKind::Const { .. })
                    })
        })
    {
        return true;
    }
    // Enum case constant used bare (`let s: Signal = Red`).
    for definition in program.data_definitions() {
        for member in program.data_members(definition) {
            if let DataMember::Variant(variant) = member
                && variant.name.as_str() == name
            {
                return true;
            }
        }
    }
    crate::locals::state_binding_type(program, machine, state, prior_statements, root, name)
        .is_some()
}

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
