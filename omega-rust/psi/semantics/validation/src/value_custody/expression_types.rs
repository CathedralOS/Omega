use diagnostics::Diagnostic;
use std::fmt;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

mod walk;
pub(crate) use walk::collect_expression_nodes;

mod cast_validation;
mod float_cast_proofs;
mod float_destinations;
mod match_dispatch;
mod operator_validation;
pub(crate) use match_dispatch::match_children;
pub use match_dispatch::match_subject_primitive_type;
pub use match_dispatch::validate_match_dispatch;
pub use match_dispatch::{
    MatchCaseDispatch, MatchCaseDispatchArm, MatchCaseSubject, match_case_dispatch,
};
pub use match_dispatch::{fresh_payloadless_case, is_fresh_payloadless_structural_value};
pub use result_type::{
    arithmetic_result_type_reference, expression_result_type_reference,
    join_result_type_references, parameter_expression_result_type_reference,
};
mod reference_values;
pub(crate) use reference_values::place_forwards_mutable_reference;
pub(crate) use result_type::domain_expression_result_type_reference;
mod result_type;
mod shape_validation;
mod value_classification;

pub(crate) use cast_validation::validate_cast_types;

pub(crate) use operator_validation::{
    report_non_bool_logical_not, report_non_integer_bitwise_not, validate_binary_operand_types,
};

pub(crate) use shape_validation::{
    report_array_scalar_shape_mismatch, report_scalar_data_shape_mismatch,
};

#[allow(unused_imports)]
pub(crate) use value_classification::ValueClass;
pub(crate) use value_classification::{
    report_cross_class_store, report_data_type_conflict, value_concrete_data_symbol,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExpressionTypeOwner<'program> {
    StateTerminalExpression {
        machine: &'program str,
        state: &'program str,
    },
}

impl fmt::Display for ExpressionTypeOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateTerminalExpression { machine, state } => {
                write!(
                    formatter,
                    "machine `{machine}` state `{state}` terminal expression"
                )
            }
        }
    }
}

/// Recheck one typed argument against an exact declared type.
///
/// Compiler-internal consumers such as package admission use this after the
/// main validation pass to reject typed-tree state that no longer agrees with
/// the declaration which originally admitted it.
pub fn argument_matches_type_reference_handle(
    program: &TypedTrees,
    argument: ExpressionHandle,
    type_reference: TypeReferenceHandle,
) -> bool {
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(argument) {
        if matches!(
            program.type_reference_table.type_reference(type_reference),
            TypeReferenceNode::Reference { .. }
        ) {
            return false;
        }
        let arms = program.expression_table.match_arms(dispatch.arms);
        return !arms.is_empty()
            && arms.iter().all(|arm| {
                argument_matches_type_reference_handle(program, arm.value, type_reference)
            });
    }
    if let ExpressionNode::Borrow(inner_expression) = program.expression_table.expression(argument)
    {
        let TypeReferenceNode::Reference {
            referee, access, ..
        } = program.type_reference_table.type_reference(type_reference)
        else {
            return false;
        };
        return inner_expression.access == *access
            && argument_matches_type_reference_handle(program, inner_expression.target, *referee);
    }

    // A resolved reference result has an exact referee type. Do not let the
    // permissive scalar-call or implicit-shared syntax fallbacks erase it.
    if let ExpressionNode::Call(call) = program.expression_table.expression(argument)
        && let Some(actual) = crate::machine_calls::calls::resolved_call_result_type(program, call)
        && matches!(
            program.type_reference_table.type_reference(actual),
            TypeReferenceNode::Reference { .. }
        )
    {
        return reference_values::reference_type_matches(program, actual, type_reference, &[]);
    }

    // An element projection selects a stored value; a range constructs a view
    // and retains the existing slice-matching path below.
    let selects_stored_value = match program.expression_table.expression(argument) {
        ExpressionNode::Member(_) => true,
        ExpressionNode::Indexed(indexed) => !matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        ),
        _ => false,
    };
    if selects_stored_value
        && matches!(
            program.type_reference_table.type_reference(type_reference),
            TypeReferenceNode::Reference { .. }
        )
    {
        return reference_values::projected_matches_reference(program, argument, type_reference);
    }

    // A reference already stored in a named parameter/local is a value of its
    // declared reference type. Forwarding that value does not form a new loan,
    // so it has no `Borrow` syntax node to inspect. Use the same exact reference
    // and array-view correspondence as projected and call-produced references.
    // Falling through to permissive name matching could change element types;
    // requiring whole reference identity would reject ordinary array views.
    if matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Reference { .. }
    ) && let ExpressionNode::Name(path) = program.expression_table.expression(argument)
        && let Some(actual) = named_value_type_reference(program, path)
        && matches!(
            program.type_reference_table.type_reference(actual),
            TypeReferenceNode::Reference { .. }
        )
    {
        // An authored shared &T slot remains open during generic-call
        // inference, as the Named type-parameter case below does. It is not a
        // closed nominal mismatch: application checking must still bind T.
        // Preserve this existing shared inference path without allowing a
        // concrete referee mismatch or widening write-only permission.
        if let TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Shared,
            referee,
            ..
        } = program.type_reference_table.type_reference(type_reference)
            && let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(*referee)
            && symbol.is_valid()
            && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
            && matches!(
                program.type_reference_table.type_reference(actual),
                TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::Shared
                        | language_semantics::ReferenceAccess::Mutable,
                    ..
                }
            )
        {
            return true;
        }
        return reference_values::reference_type_matches(program, actual, type_reference, &[]);
    }

    let argument_node = program.expression_table.expression(argument);

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, access, ..
        } => {
            let implicit_shared_source_has_identity = matches!(
                argument_node,
                ExpressionNode::Call(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
                    | ExpressionNode::String(_)
            );
            *access == language_semantics::ReferenceAccess::Shared
                && implicit_shared_source_has_identity
                && argument_matches_type_reference_handle(program, argument, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            // A literal may directly establish an owned bounded text carrier
            // (`[u8; N] in Utf8`) in argument and terminal-result positions,
            // just as it already can in field/local writes. Do this before
            // erasing the value-domain constraint: the unconstrained base is
            // an always-full fixed array, which correctly does NOT accept a
            // text literal by itself.
            (matches!(argument_node, ExpressionNode::String(literal)
                if bounded_byte_buffer_capacity(program, type_reference)
                    .is_some_and(|capacity| literal.len() <= capacity))
                || argument_matches_type_reference_handle(program, argument, *base_type))
        }
        TypeReferenceNode::FixedArray { .. } => matches!(
            argument_node,
            ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReferenceNode::Slice { element_type } => {
            // A string literal is a byte sequence, so it satisfies a `[u8]` slice
            // target (`&[u8] in Utf8 = "..."`) -- the basis for migrating string
            // literals to the `[u8] in Utf8` view. Other element types keep the
            // reference/place forms only.
            let element_is_u8 = matches!(
                program.type_reference_table.primitive_type(*element_type),
                Some(PrimitiveType::U8)
            );
            matches!(
                argument_node,
                ExpressionNode::Call(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
            ) || (element_is_u8 && matches!(argument_node, ExpressionNode::String(_)))
        }
        TypeReferenceNode::Generic { .. } => matches!(
            argument_node,
            ExpressionNode::Binary(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::Unary(_)
        ),
        TypeReferenceNode::DynamicTrait { .. } => matches!(
            argument_node,
            ExpressionNode::Call(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReferenceNode::Named {
            symbol,
            name: type_name,
        } => {
            if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
                if crate::value_custody::literals::land_anonymous_integer_expression(
                    program,
                    argument,
                    primitive_type,
                    |expression| {
                        crate::value_custody::literals::has_anonymous_operator_meaning(
                            program, expression,
                        )
                    },
                )
                .is_some()
                {
                    return true;
                }
                return matches!(argument_node, ExpressionNode::Boolean(_))
                    && primitive_type == PrimitiveType::Bool
                    || matches!(argument_node, ExpressionNode::Float(_))
                        && primitive_type.accepts_float_literal()
                    || matches!(argument_node, ExpressionNode::Integer(_))
                        && primitive_type.accepts_integer_literal()
                    || matches!(argument_node, ExpressionNode::Unary(unary)
                    if match unary.operator {
                        typed_trees::expression::UnaryOperator::BitwiseNot => {
                            primitive_type.accepts_integer_literal()
                        }
                        typed_trees::expression::UnaryOperator::LogicalNot => {
                            primitive_type == PrimitiveType::Bool
                        }
                    })
                    || matches!(
                        argument_node,
                        ExpressionNode::Binary(_)
                            | ExpressionNode::Call(_)
                            | ExpressionNode::Cast(_)
                            | ExpressionNode::Indexed(_)
                            | ExpressionNode::Member(_)
                            | ExpressionNode::Name(_)
                            | ExpressionNode::StructLiteral(_)
                    );
            }

            // Constructed nominal values keep their selected declaration even
            // when substitution copied a constant from another source module.
            // Equal spelling or layout cannot authorize a different destination.
            if symbol.is_valid() && program.symbols.get(*symbol).kind == symbols::SymbolKind::Data {
                match argument_node {
                    ExpressionNode::StructLiteral(literal) => {
                        return literal.type_symbol.is_valid() && literal.type_symbol == *symbol;
                    }
                    ExpressionNode::Name(path)
                        if program.symbols.get(path.symbol).kind
                            == symbols::SymbolKind::Variant =>
                    {
                        return program.symbols.get(path.symbol).parent == *symbol;
                    }
                    _ => {}
                }
            }

            // A named type parameter is an open slot, not a nominal data
            // declaration. Exact generic-call/operator application checking
            // separately closes that slot from the operand tuple and rejects
            // disagreement. A width-landed integer is therefore a valid
            // inference source even while this declaration still spells `T`.
            if symbol.is_valid()
                && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
                && matches!(argument_node, ExpressionNode::Integer(_))
            {
                return true;
            }

            matches!(
                argument_node,
                ExpressionNode::Binary(_)
                    | ExpressionNode::Call(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
                    | ExpressionNode::StructLiteral(_)
                    | ExpressionNode::Unary(_)
            )
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => false,
    }
}

pub(crate) fn named_value_type_reference(
    program: &TypedTrees,
    path: &typed_trees::expression::TableNamePath,
) -> Option<TypeReferenceHandle> {
    let [_] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    // Static value binders live in the shared declaration arena, including
    // domain telescopes with no executable machine/state. Their exact symbol
    // supplies the declared carrier; a same-spelled binder supplies nothing.
    if path.symbol.is_valid()
        && path.head_symbol == path.symbol
        && let Some((_, parameter)) = program
            .data_type_parameters
            .iter()
            .find(|(_, parameter)| parameter.symbol == path.symbol)
        && let typed_trees::data::TypeParameterKind::Const { type_reference }
        | typed_trees::data::TypeParameterKind::Value { type_reference } = parameter.kind
    {
        return Some(type_reference);
    }
    let matches_symbol = |candidate: symbols::SymbolHandle| {
        candidate.is_valid()
            && ((path.symbol.is_valid() && candidate == path.symbol)
                || (path.head_symbol.is_valid() && candidate == path.head_symbol))
    };

    // The resolved binder's retained symbol parent names its owning
    // declaration: machine-owned data under the machine, parameters and
    // locals under the state, parameters under the proposition. Check that
    // one container before the whole-program scan; a miss still runs the
    // full scan below, so binders whose retained parent disagrees with
    // storage keep resolving exactly as before.
    if let Some(type_reference) = hinted_named_value_type_reference(program, path, matches_symbol) {
        return Some(type_reference);
    }

    for machine in program.machines() {
        if let Some(owned) = program
            .machine_owned_data(machine)
            .iter()
            .find(|owned| matches_symbol(owned.symbol))
        {
            return Some(owned.type_reference);
        }
        for state in program.machine_states(machine) {
            if let Some(parameter) = program
                .state_parameters(state)
                .iter()
                .find(|parameter| matches_symbol(parameter.symbol))
            {
                return Some(parameter.type_reference);
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::LocalData(local) = statement
                    && matches_symbol(local.symbol)
                {
                    return Some(local.type_reference);
                }
            }
        }
    }
    for proposition in program.propositions() {
        if let Some(parameter) = program
            .proposition_parameters(proposition)
            .iter()
            .find(|parameter| matches_symbol(parameter.symbol))
        {
            return Some(parameter.type_reference);
        }
    }
    None
}

/// Parent-hinted lookup for [`named_value_type_reference`]. The retained
/// parent resolves the owning declaration in one pass over the machine and
/// proposition rosters instead of descending every state's parameters and
/// statements. Each hinted container is verified with the caller's exact
/// symbol predicate; `None` leaves the whole-program scan authoritative.
fn hinted_named_value_type_reference(
    program: &TypedTrees,
    path: &typed_trees::expression::TableNamePath,
    matches_symbol: impl Fn(symbols::SymbolHandle) -> bool,
) -> Option<TypeReferenceHandle> {
    let binder = if path.symbol.is_valid() {
        path.symbol
    } else {
        path.head_symbol
    };
    if !binder.is_valid() {
        return None;
    }
    let parent = program.symbols.get(binder).parent;
    if !parent.is_valid() {
        return None;
    }
    // Machine-owned data: the binder's parent is the machine itself.
    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == parent)
    {
        return program
            .machine_owned_data(machine)
            .iter()
            .find(|owned| matches_symbol(owned.symbol))
            .map(|owned| owned.type_reference);
    }
    // Proposition parameters: the binder's parent is the proposition.
    if let Some(proposition) = program
        .propositions()
        .iter()
        .find(|proposition| proposition.symbol == parent)
    {
        return program
            .proposition_parameters(proposition)
            .iter()
            .find(|parameter| matches_symbol(parameter.symbol))
            .map(|parameter| parameter.type_reference);
    }
    // State binders: the binder's parent is a state whose own retained
    // parent names its machine.
    let grandparent = program.symbols.get(parent).parent;
    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == grandparent)
        && let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == parent)
    {
        if let Some(parameter) = program
            .state_parameters(state)
            .iter()
            .find(|parameter| matches_symbol(parameter.symbol))
        {
            return Some(parameter.type_reference);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let StatementNode::LocalData(local) = statement
                && matches_symbol(local.symbol)
            {
                return Some(local.type_reference);
            }
        }
    }
    None
}

/// Mirror the backend layout classifier for an owned variable-fill text
/// carrier. A named value domain changes `[u8; N]` from an always-full fixed
/// array into `{len, bytes[N]}`; layout-policy domains do not.
/// The returned capacity is not evidence of the value's live length.
pub fn bounded_byte_buffer_capacity(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let has_value_domain = program
        .type_reference_table
        .constraints(*constraints)
        .iter()
        .any(|constraint| match constraint {
            TypeConstraintNode::Domain(name) => {
                !typed_trees::wire::is_layout_domain_constraint(name)
                    && language_semantics::CarryPermission::from_name(name.as_str()).is_none()
            }
            _ => false,
        });
    if !has_value_domain {
        return None;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
    } = program.type_reference_table.type_reference(*base_type)
    else {
        return None;
    };
    if program.type_reference_table.primitive_type(*element_type) != Some(PrimitiveType::U8) {
        return None;
    }
    match length {
        typed_trees::types::FixedArrayLength::Literal(capacity) => Some(*capacity),
        typed_trees::types::FixedArrayLength::ConstParameter { .. }
        | typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
    }
}

pub(crate) fn validate_expression_type_handle(
    program: &TypedTrees,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: ExpressionTypeOwner<'_>,
) {
    if let ExpressionNode::String(literal) = program.expression_table.expression(expression)
        && let Some(capacity) = bounded_byte_buffer_capacity(program, type_reference)
        && literal.len() > capacity
    {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} constructs {} byte(s), exceeding the {capacity}-byte capacity of `{}`",
            literal.len(),
            program.display_type_reference_with_constraints(type_reference),
        )));
        return;
    }
    if !argument_matches_type_reference_handle(program, expression, type_reference) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} expects `{}`, got `{}`",
            program.display_type_reference_with_constraints(type_reference),
            expression_type_name_handle(program, expression)
        )));
    }
}

pub(crate) fn expression_type_name_handle(
    program: &TypedTrees,
    argument: ExpressionHandle,
) -> &'static str {
    match program.expression_table.expression(argument) {
        ExpressionNode::Match(_) => "match expression",
        ExpressionNode::Atomic(atomic) => expression_type_name_handle(program, atomic.value),
        ExpressionNode::ArrayLiteral(_) => "array literal",
        ExpressionNode::Binary(_) => "binary expression",
        ExpressionNode::Boolean(_) => "bool",
        ExpressionNode::Call(_) => "call expression",
        ExpressionNode::Cast(_) => "cast expression",
        ExpressionNode::Float(_) => "float literal",
        ExpressionNode::Indexed(_) => "indexed value",
        ExpressionNode::Integer(_) => "integer literal",
        ExpressionNode::Member(_) => "member access",
        ExpressionNode::Borrow(inner_expression) => {
            expression_type_name_handle(program, inner_expression.target)
        }
        ExpressionNode::Name(_) => "named value",
        ExpressionNode::Range(_) => "range expression",
        ExpressionNode::StructLiteral(_) => "struct literal",
        ExpressionNode::String(_) => "String",
        ExpressionNode::Unary(unary) => match unary.operator {
            typed_trees::expression::UnaryOperator::BitwiseNot => "integer",
            typed_trees::expression::UnaryOperator::LogicalNot => "bool",
        },
        ExpressionNode::ZeroValue(_) => "zero-value representation observation",
    }
}

#[cfg(test)]
mod tests {
    use super::named_value_type_reference;
    use arena::HandleSpan;
    use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableNamePath};
    use typed_trees::machine::{Machine, OwnedData};
    use typed_trees::name::Identifier;
    use typed_trees::proposition::PropositionDefinition;
    use typed_trees::signature::StateParameter;
    use typed_trees::state::State;
    use typed_trees::statement::StatementNode;
    use typed_trees::types::TypeReferenceHandle;

    fn typed_source(source: &str) -> TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap_or_else(|diagnostics| panic!("tokenize: {diagnostics:#?}\n{source}"));
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add("named_value_type_reference.omg".into(), source.to_owned())
            .source_id;
        let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
            .unwrap_or_else(|diagnostics| panic!("parse: {diagnostics:#?}\n{source}"));
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: &syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
            },
        )
        .unwrap_or_else(|diagnostics| panic!("resolve: {diagnostics:#?}\n{source}"));
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .unwrap_or_else(|diagnostics| panic!("type: {diagnostics:#?}\n{source}"))
    }

    fn name_path(
        program: &mut TypedTrees,
        spelling: &str,
        symbol: SymbolHandle,
        head_symbol: SymbolHandle,
    ) -> TableNamePath {
        let mut members = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated(spelling));
        let mut member_symbols = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, symbol);
        TableNamePath {
            members,
            member_symbols,
            head_symbol,
            symbol,
        }
    }

    #[test]
    fn retained_parent_resolves_state_parameter_and_local_binders() {
        let program = typed_source(
            "machine inspect(index: u64) {
                 let base: u64 = index;
                 let copy: u64 = base;
             }",
        );
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let statements = program.statement_table.statements(state.statement_nodes);
        let local = |name: &str| {
            statements
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.name.as_str() == name => Some(local),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("local `{name}`"))
        };
        let ExpressionNode::Name(parameter_path) = program
            .expression_table
            .expression(local("base").initial_value)
        else {
            panic!("parameter initializer should be a name")
        };
        let index = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.name.as_str() == "index")
            .expect("index parameter");
        assert_eq!(
            named_value_type_reference(&program, parameter_path),
            Some(index.type_reference)
        );
        let ExpressionNode::Name(local_path) = program
            .expression_table
            .expression(local("copy").initial_value)
        else {
            panic!("local initializer should be a name")
        };
        assert_eq!(
            named_value_type_reference(&program, local_path),
            Some(local("base").type_reference)
        );
    }

    #[test]
    fn retained_parent_resolves_machine_owned_data() {
        let mut symbols = SymbolTableBuilder::new();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Borrowed("root"));
        let machines = symbols.insert_children(
            root,
            [(SymbolKind::Machine, SymbolNameRef::Borrowed("inspect"))],
        );
        let machine_symbol = SymbolTableBuilder::child_handles(machines)
            .next()
            .expect("machine");
        let owned = symbols.insert_children(
            machine_symbol,
            [(SymbolKind::Field, SymbolNameRef::Borrowed("counter"))],
        );
        let owned_symbol = SymbolTableBuilder::child_handles(owned)
            .next()
            .expect("owned data");
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..TypedTrees::default()
        };
        let mut machine = Machine {
            symbol: machine_symbol,
            ..Machine::default()
        };
        let type_reference = TypeReferenceHandle::from_arena_index(7);
        program.push_machine_owned_data(
            &mut machine,
            OwnedData {
                symbol: owned_symbol,
                name: Identifier::generated("counter"),
                type_reference,
                initial_value: ExpressionHandle::invalid(),
            },
        );
        program.push_machine(machine);
        let path = name_path(&mut program, "counter", owned_symbol, owned_symbol);
        assert_eq!(
            named_value_type_reference(&program, &path),
            Some(type_reference)
        );
    }

    #[test]
    fn retained_parent_resolves_proposition_parameters() {
        let mut symbols = SymbolTableBuilder::new();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Borrowed("root"));
        let propositions = symbols.insert_children(
            root,
            [(SymbolKind::Proposition, SymbolNameRef::Borrowed("bounded"))],
        );
        let proposition_symbol = SymbolTableBuilder::child_handles(propositions)
            .next()
            .expect("proposition");
        let parameters = symbols.insert_children(
            proposition_symbol,
            [(SymbolKind::Parameter, SymbolNameRef::Borrowed("limit"))],
        );
        let parameter_symbol = SymbolTableBuilder::child_handles(parameters)
            .next()
            .expect("parameter");
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..TypedTrees::default()
        };
        let mut proposition = PropositionDefinition {
            symbol: proposition_symbol,
            name: Identifier::generated("bounded"),
            ..PropositionDefinition::default()
        };
        let type_reference = TypeReferenceHandle::from_arena_index(9);
        program.push_proposition_parameter(
            &mut proposition,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated("limit"),
                type_reference,
                ..StateParameter::default()
            },
        );
        program.push_proposition(proposition);
        let path = name_path(&mut program, "limit", parameter_symbol, parameter_symbol);
        assert_eq!(
            named_value_type_reference(&program, &path),
            Some(type_reference)
        );
    }

    #[test]
    fn retained_parent_falls_back_when_it_disagrees_with_storage() {
        // Symbols claim `value` is a parameter of `retained`'s state, but the
        // parameter is actually stored under `stored`'s state. The hinted
        // container misses and the whole-program scan still resolves it,
        // matching the pre-hint contract.
        let mut symbols = SymbolTableBuilder::new();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Borrowed("root"));
        let machines = symbols.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Borrowed("stored")),
                (SymbolKind::Machine, SymbolNameRef::Borrowed("retained")),
            ],
        );
        let mut machine_symbols = SymbolTableBuilder::child_handles(machines);
        let stored_machine_symbol = machine_symbols.next().expect("stored machine");
        let retained_machine_symbol = machine_symbols.next().expect("retained machine");
        let states = symbols.insert_children(
            retained_machine_symbol,
            [(SymbolKind::State, SymbolNameRef::Borrowed("run"))],
        );
        let retained_state_symbol = SymbolTableBuilder::child_handles(states)
            .next()
            .expect("retained state");
        let parameters = symbols.insert_children(
            retained_state_symbol,
            [(SymbolKind::Parameter, SymbolNameRef::Borrowed("value"))],
        );
        let parameter_symbol = SymbolTableBuilder::child_handles(parameters)
            .next()
            .expect("parameter");

        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..TypedTrees::default()
        };
        let mut stored_machine = Machine {
            symbol: stored_machine_symbol,
            ..Machine::default()
        };
        let mut stored_state = State {
            symbol: SymbolHandle::from_arena_index(90),
            ..State::default()
        };
        let type_reference = TypeReferenceHandle::from_arena_index(11);
        program.push_state_parameter(
            &mut stored_state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated("value"),
                type_reference,
                ..StateParameter::default()
            },
        );
        program.push_machine_state(&mut stored_machine, stored_state);
        program.push_machine(stored_machine);
        // The retained container exists but does not hold the binder.
        let mut retained_machine = Machine {
            symbol: retained_machine_symbol,
            ..Machine::default()
        };
        let mut retained_state = State {
            symbol: retained_state_symbol,
            ..State::default()
        };
        program.push_state_parameter(
            &mut retained_state,
            StateParameter {
                symbol: SymbolHandle::from_arena_index(91),
                name: Identifier::generated("other"),
                type_reference: TypeReferenceHandle::from_arena_index(12),
                ..StateParameter::default()
            },
        );
        program.push_machine_state(&mut retained_machine, retained_state);
        program.push_machine(retained_machine);

        let path = name_path(&mut program, "value", parameter_symbol, parameter_symbol);
        assert_eq!(
            named_value_type_reference(&program, &path),
            Some(type_reference)
        );
    }

    #[test]
    fn retained_parent_uses_head_symbol_when_leaf_is_invalid() {
        let program = typed_source(
            "machine inspect(index: u64) {
                 let base: u64 = index;
             }",
        );
        let (index_symbol, index_type) = {
            let machine = &program.machines()[0];
            let state = &program.machine_states(machine)[0];
            let index = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.name.as_str() == "index")
                .expect("index parameter");
            (index.symbol, index.type_reference)
        };
        let mut program = program;
        let path = name_path(&mut program, "index", SymbolHandle::invalid(), index_symbol);
        assert_eq!(
            named_value_type_reference(&program, &path),
            Some(index_type)
        );
    }
}
