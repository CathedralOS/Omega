//! Operator receiver typing under the exact executable statement origin.
//!
//! Recover already-typed operands without borrowing an arithmetic destination:
//! wholly anonymous trees keep exact meaning until their completed value lands.
//! Statement-local places and explicit catalog landing supply candidate shells.
//! Computed cast and selected operator policy belong to the shared result-type
//! owner; candidate lookup never manufactures an adapted result from a peer.

use checked_trees::CheckedValueOrigin;
use numerics::literals::FloatFormat;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Recover an existing operand type handle in its executable statement context.
/// Non-statement origins have no receiver-typing context and return absence.
pub fn expression_type_reference_for_origin(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
) -> Option<TypeReferenceHandle> {
    let (state_symbol, statement_index) = match origin {
        CheckedValueOrigin::StateStatement {
            state_symbol,
            statement_index,
            ..
        } => (state_symbol, statement_index),
        CheckedValueOrigin::NestedExpression { .. }
        | CheckedValueOrigin::MachineDecrease { .. }
        | CheckedValueOrigin::MachineOwnedDataInitializer { .. } => return None,
    };
    expression_type_reference_in_state(program, state_symbol, statement_index, expression)
}

fn expression_type_reference_in_state(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .find_map(|arm| {
                expression_type_reference_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    arm.value,
                )
            }),
        ExpressionNode::Atomic(atomic) => {
            expression_type_reference_in_state(program, state_symbol, statement_index, atomic.value)
        }
        ExpressionNode::Borrow(inner) => {
            expression_type_reference_in_state(program, state_symbol, statement_index, inner.target)
        }
        ExpressionNode::Name(path) => {
            let name = program
                .expression_table
                .name_path_members(path.members)
                .first()
                .map(|name| name.as_str());
            symbol_type_reference_in_state(
                program,
                state_symbol,
                statement_index,
                path.symbol,
                name,
            )
        }
        ExpressionNode::Member(member) => expression_type_reference_in_state(
            program,
            state_symbol,
            statement_index,
            member.receiver,
        )
        .and_then(|receiver| field_type_reference(program, receiver, member))
        .or_else(|| data_binding_field_type_reference(program, state_symbol, member)),
        ExpressionNode::Indexed(_) => program
            .machine_holding_state(state_symbol)
            .zip(program.state_by_symbol(state_symbol))
            .and_then(|(machine, state)| {
                // The shared selected-index owner distinguishes builtin
                // element projection from an authored indexing result. A
                // collection shell alone cannot replace that declaration.
                crate::expression_result_type_reference(program, machine, state, expression)
            }),
        ExpressionNode::Cast(cast) => {
            crate::value_custody::expression_types::cast_result_type_reference(program, cast)
        }
        ExpressionNode::Call(call) => program
            .machine_holding_state(state_symbol)
            .zip(program.state_by_symbol(state_symbol))
            .and_then(|(machine, state)| {
                crate::expression_result_type_reference(program, machine, state, expression)
            })
            .or_else(|| {
                typed_trees::operator::resolve_named_expression_call(program, call)
                    .map(|operator| operator.return_type)
            }),
        ExpressionNode::Binary(binary) => {
            let operands = [
                expression_type_reference_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    binary.left,
                ),
                expression_type_reference_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    binary.right,
                ),
            ];
            if operands.iter().any(Option::is_some)
                && let Some(spelling) =
                    typed_trees::operator::binary_operator_spelling(binary.operator)
            {
                let candidates = typed_trees::operator::resolve_spelling_for_operands(
                    program,
                    spelling,
                    &operands,
                    program.expression_table.source_span(expression),
                );
                if let [candidate] = candidates.as_slice() {
                    if candidate.domain.is_none() {
                        return crate::value_custody::expression_types::selected_operator_result_type_reference(
                            program, candidate.operator.symbol, candidate.operator.return_type, &operands,
                        );
                    }
                    return None;
                } else if !candidates.is_empty() {
                    return None;
                }
            }
            // A comparison's result is Boolean even when its operands select
            // Float semantics. Propagating the operand carrier here would
            // misclassify an enclosing Boolean case-pattern comparison.
            use typed_trees::expression::BinaryOperator;
            if matches!(
                binary.operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or
            ) {
                return crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                    program,
                    symbols::BuiltinTypeAtom::Bool,
                );
            }
            operands[0].or(operands[1])
        }
        ExpressionNode::UndeterminedNaN(format) => float_type_reference(program, *format),
        ExpressionNode::Float(literal) => literal
            .landing()
            .and_then(|format| float_type_reference(program, format)),
        ExpressionNode::ZeroValue(type_reference) => Some(*type_reference),
        // Construction denotes its resolved nominal type, independently of
        // its fields and the destination of an enclosing operation. Retain
        // that identity exactly as a named value of the same type would.
        ExpressionNode::StructLiteral(literal) => program
            .type_reference_table
            .find_named_type_reference(literal.type_symbol),
        ExpressionNode::ArrayLiteral(_) => crate::declared_constant_array_type(program, expression),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::Unary(_) => None,
    }
}

/// Explicit catalog calls may select a format from their destination. That
/// context does not type anonymous operands of an ordinary arithmetic tree.
fn float_type_reference(
    program: &dyn crate::CheckingInput,
    format: FloatFormat,
) -> Option<TypeReferenceHandle> {
    let primitive = match format {
        FloatFormat::F32 => typed_trees::types::PrimitiveType::F32,
        FloatFormat::F64 => typed_trees::types::PrimitiveType::F64,
    };
    let atom = match primitive {
        typed_trees::types::PrimitiveType::F32 => symbols::BuiltinTypeAtom::F32,
        typed_trees::types::PrimitiveType::F64 => symbols::BuiltinTypeAtom::F64,
        _ => return None,
    };
    crate::proof_contracts::bound_expression_meaning::builtin_type_reference(program, atom)
}

fn symbol_type_reference_in_state(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    symbol: SymbolHandle,
    name: Option<&str>,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() && name.is_none() {
        return None;
    }

    let state = program.state_by_symbol(state_symbol)?;
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| {
            (symbol.is_valid() && parameter.symbol == symbol)
                || (!symbol.is_valid() && name.is_some_and(|name| parameter.name.as_str() == name))
        })
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            local_type_reference_before_statement(program, state, statement_index, symbol, name)
        })
}

fn local_type_reference_before_statement(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
    name: Option<&str>,
) -> Option<TypeReferenceHandle> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| {
            let local = statement.as_local_data()?;
            ((symbol.is_valid() && local.symbol == symbol)
                || (!symbol.is_valid() && name.is_some_and(|name| local.name.as_str() == name)))
            .then_some(local.type_reference)
        })
}

/// Field type for a member read rooted at the state's own data binding.
///
/// A binding's declared type reference does not always reach the record it
/// names: a conformance body spells its subject `&Self`, which resolves to no
/// data declaration, so `field_type_reference` cannot type `subject.value`.
/// Resolve the field through the machine's attached data definition instead.
/// `data_binding_parameter` owns which parameter holds that record; this gate
/// was keyed to `StateParameter::is_self`, which stage 01 rejects, so it
/// refused for every program.
fn data_binding_field_type_reference(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<TypeReferenceHandle> {
    let path = program.expression_table.name_path_node(member.receiver)?;
    let (machine, state) = program
        .machine_holding_state(state_symbol)
        .zip(program.state_by_symbol(state_symbol))?;
    let data_binding = program.data_binding_parameter(machine, program.state_parameters(state))?;
    let path_members = program.expression_table.name_path_members(path.members);
    // The binding is an ordinary parameter, so resolution carries its symbol;
    // the head is consulted for a path whose leaf symbol names the field.
    let names_data_binding = path_members.len() == 1
        && ((path.symbol.is_valid() && path.symbol == data_binding.symbol)
            || (path.head_symbol.is_valid() && path.head_symbol == data_binding.symbol));
    if !names_data_binding {
        return None;
    }

    let data = program.data_definition_by_symbol(machine.attached_data_symbol)?;
    data_field_type_reference(program, data, member)
}

fn field_type_reference(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<TypeReferenceHandle> {
    if program.closed_data_definition(type_reference).is_some() {
        return crate::value_custody::places::applied_data_member_field(
            program,
            type_reference,
            member,
        )
        .map(|field| field.type_reference);
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => field_type_reference(program, *referee, member),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            let data = crate::field_domains::data_definition_by_symbol_or_name(
                program,
                *base_symbol,
                base_name,
            )?;
            let field = data_field_type_reference(program, data, member)?;
            let parameters = program.data_type_parameters(data);
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            if parameters.len() != arguments.len() {
                return None;
            }
            let substitutions = parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument))
                .collect::<Vec<_>>();
            // This API returns an existing type handle, not an applied type
            // view. A raw telescope field cannot stand in for its substituted
            // type and exclude a genuinely applicable operator candidate.
            (program.normalized_type_identity(field)
                == program.type_identity(TypeIdentityRequest {
                    substitutions: &substitutions,
                    ..TypeIdentityRequest::ordinary(field)
                }))
            .then_some(field)
        }
        TypeReferenceNode::Named {
            symbol: base_symbol,
            name: base_name,
        } => crate::field_domains::data_definition_by_symbol_or_name(
            program,
            *base_symbol,
            base_name,
        )
        .and_then(|data| data_field_type_reference(program, data, member)),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn data_field_type_reference(
    program: &dyn crate::CheckingInput,
    data: &typed_trees::data::DataDefinition,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<TypeReferenceHandle> {
    crate::value_custody::places::exact_data_member_field(
        program,
        data,
        member.member_symbol,
        member.member.as_str(),
        member.case_variant.as_ref().map(|variant| variant.as_str()),
    )
    .map(|field| field.type_reference)
}
