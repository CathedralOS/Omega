use ::symbols::SymbolHandle;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn machine_state_count(program: &typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum()
}

// The machine, state and data-declaration lookups below all resolve through
// the indexes the program owns (`TypedTrees::declaration_indexes`): each
// slot pairs its map with the revision of the arenas it was built from, so
// a mutation — including the machines monomorphization appends mid-compile —
// rebuilds the index on the next query, and a dropped program can never lend
// a replacement its verdicts.

pub(crate) fn machine_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    program.machine_by_symbol(symbol)
}

pub(crate) fn state_index_by_symbol(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    symbol: SymbolHandle,
) -> Option<usize> {
    program.state_position_by_symbol(machine.symbol, symbol)
}

pub(crate) fn data_definition_by_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    program.data_definition_by_symbol(symbol)
}

pub(crate) fn state_by_symbol<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &'program typed_trees::machine::Machine,
    symbol: SymbolHandle,
) -> Option<&'program typed_trees::state::State> {
    state_index_by_symbol(program, machine, symbol)
        .map(|index| &program.machine_states(machine)[index])
}

pub(crate) fn machine_state_by_symbol(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    let machine = machine_by_symbol(program, machine_symbol)?;
    let state = state_by_symbol(program, machine, state_symbol)?;
    Some((machine, state))
}

pub(crate) fn machine_symbol_from_type_reference_handle(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            machine_symbol_from_type_reference_handle(program, *referee)
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            machine_symbol_from_type_reference_handle(program, *base_type)
        }
        typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
        | typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            ..
        } => *base_symbol,
        // A `dyn Trait<Args>` selector names its trait; the implementation it
        // selects is a runtime value, never a property of the type.
        typed_trees::types::TypeReferenceNode::DynamicTrait {
            symbol: trait_symbol,
            ..
        } => *trait_symbol,
        typed_trees::types::TypeReferenceNode::ConstExpression(_)
        | typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | typed_trees::types::TypeReferenceNode::Slice { .. }
        | typed_trees::types::TypeReferenceNode::Unit => SymbolHandle::invalid(),
    }
}

pub(crate) fn expression_root_symbol(
    expression: ExpressionHandle,
    expressions: &typed_trees::expression::ExpressionTable,
    machine_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    match expressions.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_root_symbol(indexed.collection, expressions, machine_symbol)
        }
        ExpressionNode::Borrow(inner) => {
            expression_root_symbol(inner.target, expressions, machine_symbol)
        }
        ExpressionNode::Member(member) => match expressions.expression(member.receiver) {
            ExpressionNode::Name(path)
                if path.members.count() == 1
                    && path.symbol.is_valid()
                    && path.symbol == machine_symbol =>
            {
                member
                    .member_symbol
                    .is_valid()
                    .then_some(member.member_symbol)
            }
            _ => expression_root_symbol(member.receiver, expressions, machine_symbol),
        },
        ExpressionNode::Name(path) => first_valid_name_path_symbol(path, expressions),
        _ => None,
    }
}

pub(crate) fn first_valid_name_path_symbol(
    path: &typed_trees::expression::TableNamePath,
    expressions: &typed_trees::expression::ExpressionTable,
) -> Option<SymbolHandle> {
    expressions
        .name_path_member_symbols(path.member_symbols)
        .first()
        .copied()
        .filter(|symbol| symbol.is_valid())
        .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
        .or_else(|| path.symbol.is_valid().then_some(path.symbol))
}

/// The nearest `let` of a bare name before a call boundary: state parameters
/// shadow nothing, and a shadowed local re-derives its binding by scope —
/// the same rule `resolve_runtime_subject` applies during selection.
pub(crate) fn nearest_scoped_local(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    scope_limit: usize,
    path: &[typed_trees::name::Identifier],
) -> Option<SymbolHandle> {
    let [name] = path else {
        return None;
    };
    let mut resolved = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == name.as_str())
        .map(|parameter| parameter.symbol);
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(scope_limit)
    {
        let typed_trees::statement::StatementNode::LocalData(local) = statement else {
            continue;
        };
        if local.name.as_str() == name.as_str() {
            resolved = Some(local.symbol);
        }
    }
    resolved
}

/// The checked identity string a place segment's member symbol carries: the
/// field's declared `#identity` when present, else its authored name — the
/// spelling `structural_parameter_place` produces, so a transported leaf
/// resolves to the same canonical storage.
pub(crate) fn structural_member_identity(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<String> {
    program.data_definitions().iter().find_map(|data| {
        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    Some(field.path_identity())
                }
                typed_trees::data::DataMember::Variant(variant) if variant.symbol == symbol => {
                    Some(variant.path_identity())
                }
                typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.symbol == symbol)
                    .map(|field| field.path_identity()),
                typed_trees::data::DataMember::Field(_) => None,
            })
    })
}

/// Data definition a type reference reaches, resolving a `Self` reference on
/// the machine that owns it.
pub(crate) fn declared_data_type<'a>(
    program: &'a typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    machine_symbol: SymbolHandle,
) -> Option<&'a typed_trees::data::DataDefinition> {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => declared_data_type(program, *referee, machine_symbol),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if symbol.is_valid()
                && program.symbols.get(*symbol).kind == symbols::SymbolKind::Data =>
        {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol && definition.name == *name)
        }
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == machine_symbol && name.as_str() == "Self" =>
        {
            let machine = crate::lookup::machine_by_symbol(program, machine_symbol)?;
            machine.attached_data_symbol.is_valid().then_some(())?;
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == machine.attached_data_symbol)
        }
        _ => None,
    }
}

/// Parameter or earlier local a single name resolves to inside a state's
/// scope — the scope-roster half of name-path root resolution after direct
/// path symbols are exhausted.
pub(crate) fn state_scoped_name_symbol(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    name: &str,
    statement_index: usize,
) -> Option<SymbolHandle> {
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == name)
    {
        return Some(parameter.symbol);
    }

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local_data)
                if local_data.name.as_str() == name =>
            {
                Some(local_data.symbol)
            }
            _ => None,
        })
}
