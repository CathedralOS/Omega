//! Exact declaration roots, nominal type heads and scoped storage names.

use ::symbols::SymbolHandle;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

pub fn machine_symbol_from_type_reference_handle(
    program: &dyn crate::CheckingInput,
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

pub fn expression_root_symbol(
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

pub fn first_valid_name_path_symbol(
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

/// A field projection admitted as a runtime value subject: the term is a
/// parameter or local in `state`, and `member` is a field of that value's
/// data type. Evidence projections that do not resolve this way stay evidence.
pub struct RuntimeFieldSubject {
    pub receiver: SymbolHandle,
    pub field: SymbolHandle,
}

pub fn runtime_field_subject(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    term: &typed_trees::name::Identifier,
    member: &typed_trees::name::Identifier,
) -> Option<RuntimeFieldSubject> {
    let receiver = nearest_scoped_local(program, state, usize::MAX, std::slice::from_ref(term))?;
    let carrier = scoped_binding_type(program, state, receiver)?;
    let data = declared_data_type(program, carrier)?;
    let field = program.data_members(data).iter().find_map(|row| {
        let typed_trees::data::DataMember::Field(field) = row else {
            return None;
        };
        (field.name.as_str() == member.as_str()).then_some(field.symbol)
    })?;
    Some(RuntimeFieldSubject { receiver, field })
}

fn scoped_binding_type(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
    {
        return Some(parameter.type_reference);
    }
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .rev()
        .find_map(|statement| {
            let local = statement.as_local_data()?;
            (local.symbol == symbol).then_some(local.type_reference)
        })
}

/// The nearest `let` of a bare name before a call boundary: state parameters
/// shadow nothing, and a shadowed local re-derives its binding by scope —
/// the same rule `resolve_runtime_subject` applies during selection.
pub fn nearest_scoped_local(
    program: &dyn crate::CheckingInput,
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
        let Some(local) = statement.as_local_data() else {
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
pub fn structural_member_identity(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<checked_trees::CheckedMemberIdentity> {
    program.data_definitions().iter().find_map(|data| {
        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    Some(field.member_identity())
                }
                typed_trees::data::DataMember::Variant(variant) if variant.symbol == symbol => {
                    Some(variant.member_identity())
                }
                typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_field_by_symbol(variant, symbol)
                    .map(|field| {
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        )
                    }),
                typed_trees::data::DataMember::Field(_) => None,
            })
    })
}

/// The field or variant member `member_name` declares on `data`, by symbol.
pub fn data_member_symbol_by_name(
    program: &dyn crate::CheckingInput,
    data: &typed_trees::data::DataDefinition,
    member_name: &str,
) -> Option<SymbolHandle> {
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == member_name => {
                Some(field.symbol)
            }
            typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == member_name =>
            {
                Some(variant.symbol)
            }
            _ => None,
        })
}

/// Data definition a type reference names, through reference and constraint
/// shells.
pub fn declared_data_type(
    program: &dyn crate::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => declared_data_type(program, *referee),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if symbol.is_valid()
                && program.symbols.get(*symbol).kind == symbols::SymbolKind::Data =>
        {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol && definition.name == *name)
        }
        _ => None,
    }
}

/// Parameter or earlier local a single name resolves to inside a state's
/// scope — the scope-roster half of name-path root resolution after direct
/// path symbols are exhausted.
pub fn state_scoped_name_symbol(
    program: &dyn crate::CheckingInput,
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
