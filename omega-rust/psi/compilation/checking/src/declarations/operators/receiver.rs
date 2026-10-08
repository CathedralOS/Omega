//! Operator receiver typing under the exact executable statement origin.
//!
//! Recover already-typed operands without borrowing an arithmetic destination:
//! wholly anonymous trees keep exact meaning until their completed value lands.
//! Explicit catalog calls, first-derivable match arms and direct cast targets
//! have their own selection rules; general result typing cannot replace them.

use checked_trees::CheckedValueOrigin;
use numerics::literals::FloatFormat;
use symbols::{BuiltinFunction, SymbolHandle};
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
        .and_then(|receiver| {
            field_type_reference(program, receiver, member.member_symbol, &member.member)
        })
        .or_else(|| data_binding_field_type_reference(program, state_symbol, member)),
        ExpressionNode::Indexed(indexed) => expression_type_reference_in_state(
            program,
            state_symbol,
            statement_index,
            indexed.collection,
        )
        .and_then(|collection| indexed_element_type_reference(program, collection)),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Call(call) => program
            .machine_holding_state(state_symbol)
            .zip(program.state_by_symbol(state_symbol))
            .and_then(|(machine, state)| {
                crate::expression_result_type_reference(program, machine, state, expression)
            })
            .or_else(|| {
                typed_trees::operator::resolve_named_expression_call(program, call)
                    .map(|operator| operator.return_type)
            })
            .or_else(|| {
                [
                    BuiltinFunction::Min,
                    BuiltinFunction::Max,
                    BuiltinFunction::Sqrt,
                ]
                .into_iter()
                .any(|function| {
                    program.symbols.builtin_function_symbol(function) == Some(call.target_symbol)
                })
                .then(|| {
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .find_map(|argument| {
                            expression_type_reference_in_state(
                                program,
                                state_symbol,
                                statement_index,
                                *argument,
                            )
                        })
                })
                .flatten()
                .or_else(|| {
                    contextual_type_reference_in_state(program, state_symbol, statement_index)
                })
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
                        // Float arithmetic retains its selected operand policy for
                        // enclosing operations; comparisons retain the declaration's
                        // Boolean result instead of borrowing an operand carrier.
                        if typed_trees::operator::primitive_float_binary_semantics(
                            program,
                            candidate.operator,
                        )
                        .is_some()
                        {
                            use checked_trees::CheckedArithmeticPolicyAdapter;
                            use numerics::arithmetic::ArithmeticDomain;
                            let domain = match super::uses::arithmetic_policy_adapter(
                                program, spelling, &operands,
                            ) {
                                CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                                    ..
                                } => ArithmeticDomain::Saturating,
                                CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                                    ..
                                } => ArithmeticDomain::Trapping,
                                _ => ArithmeticDomain::Exact,
                            };
                            let TypeReferenceNode::Named {
                                symbol: carrier, ..
                            } = program
                                .type_reference_table
                                .type_reference(candidate.operator.return_type)
                            else {
                                return None;
                            };
                            return program
                                .type_reference_table
                                .find_arithmetic_result_type_reference(*carrier, domain);
                        }
                        return Some(candidate.operator.return_type);
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
fn contextual_type_reference_in_state(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
) -> Option<TypeReferenceHandle> {
    let state = program.state_by_symbol(state_symbol)?;
    match program
        .statement_table
        .statement_at(state, statement_index)?
    {
        typed_trees::statement::StatementNode::Assignment(assignment) => {
            expression_type_reference_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            )
        }
        typed_trees::statement::StatementNode::LocalData(local) => Some(local.type_reference),
        _ => None,
    }
}

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
                || name.is_some_and(|name| parameter.name.as_str() == name)
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
                || name.is_some_and(|name| local.name.as_str() == name))
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
    program.data_members(data).iter().find_map(|data_member| {
        let field = data_member.as_field()?;
        ((member.member_symbol.is_valid() && field.symbol == member.member_symbol)
            || field.name == member.member)
            .then_some(field.type_reference)
    })
}

fn field_type_reference(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    field_symbol: SymbolHandle,
    field_name: &typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => field_type_reference(program, *referee, field_symbol, field_name),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            let data = program.data_definitions().iter().find(|data| {
                (base_symbol.is_valid() && data.symbol == *base_symbol) || data.name == *base_name
            })?;
            let field = data_field_type_reference(program, data, field_symbol, field_name)?;
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
        } => program
            .data_definitions()
            .iter()
            .find(|data| {
                (base_symbol.is_valid() && data.symbol == *base_symbol) || data.name == *base_name
            })
            .and_then(|data| data_field_type_reference(program, data, field_symbol, field_name)),
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
    field_symbol: SymbolHandle,
    field_name: &typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    let field = if field_symbol.is_valid() {
        program.data_field(data, field_symbol)
    } else {
        program.data_field_named(data, field_name.as_str())
    };
    field.map(|field| field.type_reference)
}

fn indexed_element_type_reference(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => indexed_element_type_reference(program, *referee),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some(*element_type),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => None,
    }
}
