//! Range premises use the declaration reached through the receiver's type.
//! Field identities belong to their data declaration, never a machine alias.
//! A global same-spelling search can
//! both reject a valid index and borrow another record's range to admit an
//! invalid one. Ordinary projections retain this checker's scoped root lookup.

use facts::PlaceSegment;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::data::DataDefinition;
use typed_trees::data::TypeParameter;
use typed_trees::data::TypeParameterKind;
use typed_trees::dependent_ranges::{
    DependentBoundClass, SymbolicMaxBound, dependent_range_maximum,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use typed_trees::unsigned_integer_primitive_pattern;
use validation::closed_integer_range_bound;
use validation::closed_integer_range_maximum;
use validation::declared_constant_array_type;
use validation::declared_domain_predicate_bounds;
use validation::declared_place_type_raw;
use validation::exact_attached_field;
use validation::exact_self_field;
use validation::expression_result_type_reference;
use validation::project_type_reference_from_segments;

/// Same-carrier integer casts preserve positions, not domain or loan authority.
pub(super) fn same_carrier_integer_cast_source(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) else {
        return None;
    };
    if cast.form.is_recast() {
        return None;
    }
    let target = program.primitive_type_reference(cast.target_type)?;
    let source = expression_result_type_reference(program, machine, state, cast.value)
        .and_then(|reference| program.primitive_type_reference(reference))?;
    (target.is_fixed_integer() && target == source).then_some(cast.value)
}

/// Whether `expression`'s type is provably an UNSIGNED integer primitive. An
/// unsigned index is non-negative by construction, so it discharges the lower
/// half of the index obligation (`0 <= i`) by type and never needs a separate
/// `>= 0` proof. A signed index -- or one whose type cannot be resolved -- is
/// NOT exempt (closed-world: exempt only when provably unsigned).
pub(super) fn expression_is_unsigned_integer(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    matches!(
        expression_type_reference(program, machine, state, expression)
            .and_then(|type_reference| primitive_of_type_reference(program, type_reference)),
        Some(unsigned_integer_primitive_pattern!())
    )
}

/// Carrier bounds hold under every arithmetic policy. This is deliberately
/// separate from declared refinements: Wrapping does not enforce a declared
/// interval, but its u8 result still cannot exceed 255. Signed nonnegativity
/// remains a separate obligation at the access.
pub(in crate::checks) fn expression_integer_carrier_maximum(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<u64> {
    let reference = expression_result_type_reference(program, machine, state, expression)?;
    match program.primitive_type_reference(reference)? {
        PrimitiveType::U8 => Some(u64::from(u8::MAX)),
        PrimitiveType::U16 => Some(u64::from(u16::MAX)),
        PrimitiveType::U32 => Some(u64::from(u32::MAX)),
        PrimitiveType::U64 => Some(u64::MAX),
        PrimitiveType::I8 => u64::try_from(i8::MAX).ok(),
        PrimitiveType::I16 => u64::try_from(i16::MAX).ok(),
        PrimitiveType::I32 => u64::try_from(i32::MAX).ok(),
        PrimitiveType::I64 => u64::try_from(i64::MAX).ok(),
        PrimitiveType::Bool
        | PrimitiveType::Selector
        | PrimitiveType::Addr
        | PrimitiveType::F32
        | PrimitiveType::F64 => None,
    }
}

/// The carrier floor symmetric to `expression_integer_carrier_maximum`:
/// unsigned carriers bottom at 0 and signed ones at their type minimum.
/// BigInt so `i64::MIN` and every unsigned bound share one comparison type.
/// Non-integer carriers contribute no floor.
pub(in crate::checks) fn expression_integer_carrier_minimum(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<BigInt> {
    let reference = expression_result_type_reference(program, machine, state, expression)?;
    match program.primitive_type_reference(reference)? {
        unsigned_integer_primitive_pattern!() => Some(BigInt::from_u64(0)),
        PrimitiveType::I8 => Some(BigInt::from_i64(i64::from(i8::MIN))),
        PrimitiveType::I16 => Some(BigInt::from_i64(i64::from(i16::MIN))),
        PrimitiveType::I32 => Some(BigInt::from_i64(i64::from(i32::MIN))),
        PrimitiveType::I64 => Some(BigInt::from_i64(i64::MIN)),
        PrimitiveType::Bool
        | PrimitiveType::Selector
        | PrimitiveType::Addr
        | PrimitiveType::F32
        | PrimitiveType::F64 => None,
    }
}

/// Resolves a type reference (through references and constraints) to its
/// underlying primitive, when it names one.
fn primitive_of_type_reference(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => primitive_of_type_reference(program, *referee),
        TypeReferenceNode::Named { name, .. } => PrimitiveType::from_name(name.as_str()),
        _ => None,
    }
}

pub(super) fn expression_is_slice(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    expression_type_reference(program, machine, state, expression)
        .is_some_and(|type_reference| type_reference_is_slice(program, type_reference))
}

/// The DECLARED range constraint of `expression`'s type (`i: usize [0..=4]`)
/// as inclusive i64 bounds -- ONLY when that range is a true invariant the
/// index prover may rely on:
/// - the base type is a plain INTEGER primitive (an atomic's hardware
///   semantics wrap; a float range never indexes), and
/// - the arithmetic domain is EXACT: a `in Wrapping`/`Saturating` store is
///   deliberately permissive (probed live: `[0..=4] in Wrapping` accepts a
///   store of 100), so a non-Exact range must never discharge an index bound.
///
/// Every store into an Exact ranged place is range-checked (the narrowing
/// keystone) and ZII requires 0 in range, so the declared interval holds at
/// every read. An exactly selected ordinary call contributes its declared
/// return interval and enforced result contract, intersected with its carrier.
/// Checking the callee's return remains an independent duty.
pub(in crate::checks) fn expression_enforced_declared_range(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(i64, i64)> {
    let type_reference = match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            if call.static_requirement_dispatch.is_some() {
                return None;
            }
            declared_place_type_raw(program, machine, Some(state), expression)?
        }
        _ => expression_type_reference(program, machine, state, expression)?,
    };
    if !primitive_of_type_reference(program, type_reference)?.is_fixed_integer() {
        return None;
    }
    let (low, high) =
        super::expressions::ensured_call_result_bounds(program, machine.symbol, expression)?;
    let minimum = expression_integer_carrier_minimum(program, machine, state, expression)
        .and_then(|value| value.to_i64());
    let maximum = expression_integer_carrier_maximum(program, machine, state, expression)
        .and_then(|value| i64::try_from(value).ok());
    let minimum = match (minimum, low) {
        (Some(carrier), Some(ensured)) => Some(carrier.max(ensured)),
        (carrier, ensured) => carrier.or(ensured),
    }?;
    let maximum = match (maximum, high) {
        (Some(carrier), Some(ensured)) => Some(carrier.min(ensured)),
        (carrier, ensured) => carrier.or(ensured),
    }?;
    (minimum <= maximum).then_some((minimum, maximum))
}

// A Range constraint whose minimum literal-folds and whose maximum is the
// R1a-admissible symbolic shape, under Exact shells only (mirrors
// `enforced_range_of_type_reference`'s domain kill).

/// Use the same receiver-substituted extent as ordered collection checking.
/// A closed lower bound is only a fast proof; alternative entry contracts are
/// left intact for the shared integer judge when this floor is insufficient.
pub(in crate::checks::ranges) fn collection_symbolic_extent_floor(
    program: &dyn ::validation::CheckingInput,
    state: &State,
    collection: ExpressionHandle,
    facts: &super::facts::RangeFacts<'_>,
) -> Option<i64> {
    let Ok(extent) =
        validation::resolved_array_extent_at(program, state, facts.statement_index, collection)
    else {
        return None;
    };
    match extent {
        validation::ResolvedArrayExtent::Closed(value) => value.to_i64(),
        validation::ResolvedArrayExtent::Bound {
            symbol,
            type_reference,
        } => {
            let declared =
                primitive_of_type_reference(program, type_reference).and_then(|primitive| {
                    matches!(primitive, unsigned_integer_primitive_pattern!()).then_some(0)
                });
            let required = facts.const_binder_floor(symbol);
            match (declared, required) {
                (Some(declared), Some(required)) => Some(declared.max(required)),
                (declared, required) => declared.or(required),
            }
        }
    }
}

/// Exact invocation-fixed binders alone may retain lower bounds across writes.
/// Runtime values and same-spelled locals do not identify an extent binder.
pub(in crate::checks::ranges) fn is_const_binder(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    symbol: SymbolHandle,
) -> bool {
    if !symbol.is_valid() {
        return false;
    }
    let matches = |parameter: &TypeParameter| {
        parameter.symbol == symbol && matches!(parameter.kind, TypeParameterKind::Const { .. })
    };
    program.machine_type_parameters(machine).iter().any(matches)
        || program
            .data_definition_by_symbol(machine.attached_data_symbol)
            .is_some_and(|data| program.data_type_parameters(data).iter().any(matches))
}

pub(in crate::checks::ranges) fn expression_type_reference(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Cast(_) => {
            expression_result_type_reference(program, machine, state, expression)
        }
        ExpressionNode::ArrayLiteral(_) => declared_constant_array_type(program, expression),
        ExpressionNode::StructLiteral(_) => {
            expression_result_type_reference(program, machine, state, expression)
        }
        ExpressionNode::Borrow(inner) => {
            expression_type_reference(program, machine, state, inner.target)
        }
        ExpressionNode::Call(call) => {
            // A call result's declared type is its target state's return type —
            // the same reader `expression_enforced_declared_range` uses. A
            // `u64` result is non-negative by construction, discharging the
            // lower half of an index obligation by type alone.
            if call.static_requirement_dispatch.is_some() {
                return None;
            }
            declared_place_type_raw(program, machine, Some(state), expression)
        }
        ExpressionNode::Indexed(indexed)
            if !matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) =>
        {
            let collection =
                expression_type_reference(program, machine, state, indexed.collection)?;
            project_type_reference_from_segments(
                program,
                collection,
                &[PlaceSegment::Index {
                    expression: indexed.index,
                }],
            )
        }
        // Checking refuses a value name that resolved to no in-scope binder
        // before ranges run. A same-spelled binder elsewhere in the machine is
        // a different declaration and lends no type.
        ExpressionNode::Name(path) => {
            type_reference_for_symbol(program, machine, state, path.symbol)
        }
        ExpressionNode::Member(member) => exact_self_field(program, machine, expression)
            .map(|field| field.type_reference)
            .or_else(|| {
                expression_type_reference(program, machine, state, member.receiver).and_then(
                    |receiver_type| {
                        data_field_type_reference(
                            program,
                            receiver_type,
                            member.member_symbol,
                            &member.member,
                        )
                    },
                )
            }),
        // The selected operation supplies the result carrier for both unary
        // and binary computations. For example, `~position` and `position - 1`
        // on u64 are nonnegative by their result type, not by operand bounds.
        ExpressionNode::Binary(_) | ExpressionNode::Unary(_) => {
            expression_result_type_reference(program, machine, state, expression)
        }
        _ => None,
    }
}

fn type_reference_for_symbol(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .state_parameter_by_symbol(state, symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .state_local_declaration(state, symbol)
                .map(|(_, local)| local.type_reference)
        })
        .or_else(|| {
            program
                .machine_owned_data_by_symbol(machine, symbol)
                .map(|owned| owned.type_reference)
        })
        .or_else(|| {
            // A const/value binder denotes a symbolic value of its declared
            // carrier while the template is checked, so `values[Count]`
            // proves its lower bound from `u32` like any other unsigned
            // subject. Specialization replaces the binder before runtime.
            program
                .machine_type_parameters(machine)
                .iter()
                .find_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference }
                        if parameter.symbol == symbol =>
                    {
                        Some(*type_reference)
                    }
                    _ => None,
                })
        })
        .or_else(|| {
            // State bindings are WHOLE-MACHINE scope: a param declared on the
            // machine's entry state is readable from every sub-state, so its
            // declared range must feed the prover there too (`pick(k: usize
            // [0..=3]) { ... state go { self.arr[k] } }`). Symbol match is
            // exact, so a sibling state's same-named param cannot alias.
            program.machine_states(machine).iter().find_map(|other| {
                program
                    .state_parameter_by_symbol(other, symbol)
                    .map(|parameter| parameter.type_reference)
            })
        })
}

fn type_reference_is_slice(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => type_reference_is_slice(program, *referee),
        TypeReferenceNode::Slice { .. } => true,
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

fn data_field_type_reference(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    member_symbol: SymbolHandle,
    member_name: &Identifier,
) -> Option<TypeReferenceHandle> {
    ::validation::field_domains::data_field_type_reference_by_symbol_or_name(
        program,
        type_reference,
        member_symbol,
        member_name,
        data_definition_by_symbol_or_name,
    )
}

fn data_definition_by_symbol_or_name<'program>(
    program: &'program dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
    name: &Identifier,
) -> Option<&'program DataDefinition> {
    program.data_definitions().iter().find(|data_definition| {
        if symbol.is_valid() {
            data_definition.symbol == symbol
        } else {
            data_definition.name == *name
        }
    })
}
