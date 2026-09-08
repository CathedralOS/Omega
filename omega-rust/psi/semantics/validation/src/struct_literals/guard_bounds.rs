//! Constructor guard facts for immutable owned inputs only. References and
//! effectful expressions need write-frame evidence before entering this path.

use crate::arithmetic_domains::{ValueEnv, fall_through_narrowed_env, guard_narrowed_env};
use numerics::arithmetic::ArithmeticDomain;
use symbols::BuiltinTypeAtom;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::TransitionGuardNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn construction_guard_environment(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    guard: &TransitionGuardNode,
    positive: bool,
) -> ValueEnv {
    let empty = ValueEnv::new();
    let TransitionGuardNode::When(condition) = guard else {
        return empty;
    };
    if !has_immutable_inputs(program, machine, state, *condition) {
        return empty;
    }
    if positive {
        guard_narrowed_env(program, machine, Some(state), guard, &empty)
    } else {
        fall_through_narrowed_env(program, machine, Some(state), guard, &empty)
    }
}

pub(super) fn has_immutable_inputs(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            immutable_place_type(program, machine, state, expression).is_some_and(|handle| {
                let TypeReferenceNode::Named { symbol, .. } = base_type(program, handle) else {
                    return false;
                };
                program.arithmetic_domain_for_type_reference(handle) == ArithmeticDomain::Exact
                    && matches!(
                        program.symbols.builtin_type_atom(*symbol),
                        Some(
                            BuiltinTypeAtom::U8
                                | BuiltinTypeAtom::U16
                                | BuiltinTypeAtom::U32
                                | BuiltinTypeAtom::U64
                                | BuiltinTypeAtom::I8
                                | BuiltinTypeAtom::I16
                                | BuiltinTypeAtom::I32
                                | BuiltinTypeAtom::I64
                                | BuiltinTypeAtom::Bool
                        )
                    )
            })
        }
        ExpressionNode::Binary(binary) => {
            crate::bound_expression_meaning::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) && has_immutable_inputs(program, machine, state, binary.left)
                && has_immutable_inputs(program, machine, state, binary.right)
        }
        // Logical negation is not an overloadable operator spelling. Its
        // operand still owes exact selected meaning and immutable inputs.
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            has_immutable_inputs(program, machine, state, unary.operand)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| has_immutable_inputs(program, machine, state, field.value)),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .all(|element| has_immutable_inputs(program, machine, state, *element)),
        _ => false,
    }
}

fn immutable_place_type(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let handle = match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            let [name] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            if !program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
            {
                return None;
            }
            let parameter = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol && parameter.name == *name)?;
            if parameter.is_self || parameter.is_mutable || parameter.is_const {
                return None;
            }
            parameter.type_reference
        }
        ExpressionNode::Member(member) if member.case_variant.is_none() => {
            let receiver = immutable_place_type(program, machine, state, member.receiver)?;
            let TypeReferenceNode::Named { symbol, .. } = base_type(program, receiver) else {
                return None;
            };
            let selector = program.symbols.get(member.member_symbol);
            if !member.member_symbol.is_valid()
                || selector.kind != symbols::SymbolKind::Field
                || selector.parent != *symbol
            {
                return None;
            }
            let definition = program.data_definitions().iter().find(|definition| {
                symbol.is_valid()
                    && definition.symbol == *symbol
                    && definition.type_parameters.is_empty()
            })?;
            let mut fields =
                program
                    .data_members(definition)
                    .iter()
                    .filter_map(|field| match field {
                        DataMember::Field(field)
                            if field.symbol == member.member_symbol
                                && field.name == member.member =>
                        {
                            Some(field.type_reference)
                        }
                        _ => None,
                    });
            let field_type = fields.next()?;
            if fields.next().is_some() {
                return None;
            }
            field_type
        }
        _ => return None,
    };
    matches!(base_type(program, handle), TypeReferenceNode::Named { .. }).then_some(handle)
}

fn base_type(program: &TypedTrees, handle: TypeReferenceHandle) -> &TypeReferenceNode {
    let mut base = handle;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(base)
    {
        base = *base_type;
    }
    program.type_reference_table.type_reference(base)
}
