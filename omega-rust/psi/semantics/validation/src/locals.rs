use crate::symbols::{MachineSymbols, TopLevelSymbols};
use diagnostics::Diagnostic;
use typed_trees::data::{DataMember, TypeParameterKind};
use typed_trees::signature::StateParameter;
use typed_trees::statement::StatementNode;
use typed_trees::{TypedTrees, machine::Machine, state::State};
mod type_value_scope;
mod value_scope;
pub(crate) use value_scope::StateValueScope;

/// A named operator's complete declaration namespace is not a storage read.
/// Retained value identities never acquire namespace status from spelling.
pub(crate) fn is_named_operator_namespace(
    program: &TypedTrees,
    receiver_symbols: &[symbols::SymbolHandle],
    namespace: &[&str],
    target_symbol: symbols::SymbolHandle,
    target: &str,
    argument_count: usize,
) -> bool {
    if namespace.is_empty()
        || receiver_symbols.iter().any(|symbol| {
            symbol.is_valid()
                && !matches!(
                    program.symbols.get(*symbol).kind,
                    symbols::SymbolKind::Module
                        | symbols::SymbolKind::BuiltinType
                        | symbols::SymbolKind::Data
                        | symbols::SymbolKind::Domain
                        | symbols::SymbolKind::Machine
                        | symbols::SymbolKind::Trait
                )
        })
    {
        return false;
    }
    typed_trees::operator::resolve_named_call(
        program,
        symbols::SymbolHandle::invalid(),
        Some(namespace),
        target,
        argument_count,
        false,
    )
    .is_some_and(|operator| !target_symbol.is_valid() || target_symbol == operator.symbol)
}

pub(crate) fn expression_call_has_operator_namespace(
    program: &TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    let table = &program.expression_table;
    let typed_trees::expression::ExpressionNode::Name(path) = table.expression(call.receiver)
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
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    prior_statements: &[StatementNode],
    symbol: symbols::SymbolHandle,
    name: &str,
) -> Option<typed_trees::types::TypeReferenceHandle> {
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
        symbols::SymbolKind::Parameter => program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol && parameter.name.as_str() == name)
            .map(|parameter| parameter.type_reference),
        symbols::SymbolKind::Local => {
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
    root: symbols::SymbolHandle,
    name: &str,
) -> bool {
    let kind = program.symbols.get(root).kind;
    if name == "self"
        || matches!(
            kind,
            symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
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
    if root.is_valid() && kind == symbols::SymbolKind::Unknown {
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
    pub(crate) program: &'program typed_trees::TypedTrees,
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
            && (receiver_allows_mutation(self.program, self.parameters)
                || self
                    .program
                    .machine_owned_data(self.machine)
                    .iter()
                    .any(|owned| owned.name.as_str() == root_name)))
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

/// Attached fields inherit the current state's receiver access, not ownership
/// merely from their presence in the machine's declaration namespace.
pub fn receiver_allows_mutation(program: &TypedTrees, parameters: &[StateParameter]) -> bool {
    parameters.iter().any(|parameter| {
        parameter.is_self
            && !parameter.is_const
            && parameter.type_reference.is_valid()
            && match program
                .type_reference_table
                .type_reference(parameter.type_reference)
            {
                typed_trees::types::TypeReferenceNode::Reference { access, .. } => {
                    access.is_exclusive()
                }
                _ => true,
            }
    })
}

pub(crate) fn local_is_mutable_reference(
    program: &typed_trees::TypedTrees,
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
