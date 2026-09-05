use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Whether `expression`'s type is provably an UNSIGNED integer primitive. An
/// unsigned index is non-negative by construction, so it discharges the lower
/// half of the index obligation (`0 <= i`) by type and never needs a separate
/// `>= 0` proof. A signed index -- or one whose type cannot be resolved -- is
/// NOT exempt (closed-world: exempt only when provably unsigned).
pub(super) fn expression_is_unsigned_integer(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    matches!(
        expression_type_reference(program, machine, state, expression)
            .and_then(|type_reference| primitive_of_type_reference(program, type_reference)),
        Some(PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64)
    )
}

/// Resolves a type reference (through references and constraints) to its
/// underlying primitive, when it names one.
fn primitive_of_type_reference(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
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
/// every read.
pub(in crate::checks) fn expression_enforced_declared_range(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(i64, i64)> {
    let type_reference = expression_type_reference(program, machine, state, expression)?;
    let primitive = primitive_of_type_reference(program, type_reference)?;
    if !matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    enforced_range_of_type_reference(program, type_reference)
        .or_else(|| dependent_range_substituted(program, machine, type_reference))
}

/// R1 dependent range (`i: u32 [0..=self.count]`): the SUBSTITUTED literal
/// interval. The caller-side proof obligation established `i <= count@entry
/// plus `offset`; `count`'s own enforced literal range holds at EVERY write
/// (store-enforced), so `count@entry <= high(count)` and the substituted
/// `(min, high(count) + offset)` is a true invariant of `i` -- immune to the
/// field being reassigned after entry. `None` when the named field is
/// unranged, non-Exact, or the arithmetic overflows (the index prover then
/// reports its clean cannot-prove error).
fn dependent_range_substituted(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    handle: TypeReferenceHandle,
) -> Option<(i64, i64)> {
    let (minimum, symbolic) = dependent_range_of_type_reference(program, handle)?;
    let attached = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    let field_type = program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == symbolic.field.as_str() =>
            {
                field
                    .type_reference
                    .is_valid()
                    .then_some(field.type_reference)
            }
            _ => None,
        })?;
    // The field itself must be an enforced-integer-ranged place (same gate
    // this module applies to any range an index proof may trust).
    if primitive_of_type_reference(program, field_type).is_none_or(|primitive| {
        !matches!(
            primitive,
            PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
        )
    }) {
        return None;
    }
    let (_, field_high) = enforced_range_of_type_reference(program, field_type)?;
    let substituted_high = field_high.checked_add(symbolic.offset)?;
    (minimum <= substituted_high).then_some((minimum, substituted_high))
}

/// A Range constraint whose minimum literal-folds and whose maximum is the
/// R1a-admissible symbolic shape, under Exact shells only (mirrors
/// `enforced_range_of_type_reference`'s domain kill).
fn dependent_range_of_type_reference(
    program: &typed_trees::TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<(i64, typed_trees::dependent_ranges::SymbolicMaxBound)> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            dependent_range_of_type_reference(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            if constraints.iter().any(|constraint| {
                matches!(
                    constraint,
                    typed_trees::types::TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != numerics::arithmetic::ArithmeticDomain::Exact
                )
            }) {
                return None;
            }
            constraints
                .iter()
                .find_map(|constraint| match constraint {
                    typed_trees::types::TypeConstraintNode::Range { minimum, maximum } => {
                        let minimum = program.expression_table.constant_integer_value(*minimum)?;
                        let symbolic = typed_trees::dependent_ranges::symbolic_max_bound(
                            &program.expression_table,
                            *maximum,
                        )?;
                        Some((minimum, symbolic))
                    }
                    _ => None,
                })
                .or_else(|| dependent_range_of_type_reference(program, *base_type))
        }
        _ => None,
    }
}

fn enforced_range_of_type_reference(
    program: &typed_trees::TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<(i64, i64)> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            enforced_range_of_type_reference(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            // Any non-Exact arithmetic domain in the shells kills the invariant.
            if constraints.iter().any(|constraint| {
                matches!(
                    constraint,
                    typed_trees::types::TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != numerics::arithmetic::ArithmeticDomain::Exact
                )
            }) {
                return None;
            }
            constraints
                .iter()
                .find_map(|constraint| match constraint {
                    typed_trees::types::TypeConstraintNode::Range { minimum, maximum } => Some((
                        program.expression_table.constant_integer_value(*minimum)?,
                        program.expression_table.constant_integer_value(*maximum)?,
                    )),
                    _ => None,
                })
                .or_else(|| enforced_range_of_type_reference(program, *base_type))
        }
        _ => None,
    }
}

pub(in crate::checks::ranges) fn expression_type_reference(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            expression_type_reference(program, machine, state, inner.target)
        }
        ExpressionNode::Indexed(indexed)
            if !matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) =>
        {
            let collection =
                expression_type_reference(program, machine, state, indexed.collection)?;
            crate::flow::project_type_reference_from_segments(
                program,
                collection,
                &[facts::PlaceSegment::Index {
                    expression: indexed.index,
                }],
            )
        }
        ExpressionNode::Name(path) => {
            type_reference_for_symbol(program, machine, state, path.symbol).or_else(|| {
                let name = program
                    .expression_table
                    .name_path_members(path.members)
                    .last()?;
                type_reference_for_name(program, machine, state, name)
            })
        }
        ExpressionNode::Member(member) => {
            expression_type_reference(program, machine, state, member.receiver)
                .and_then(|receiver_type| {
                    data_field_type_reference(
                        program,
                        receiver_type,
                        member.member_symbol,
                        &member.member,
                    )
                })
                .or_else(|| {
                    // `self.field`: the receiver `self` does not resolve to a type
                    // reference (attached-data fields are not in the machine's owned-data
                    // span), so resolve the field directly from its data definition. The
                    // member symbol is field-unique, so this matches the right field.
                    program.data_definitions().iter().find_map(|definition| {
                        data_field_in_definition(
                            program,
                            definition,
                            member.member_symbol,
                            &member.member,
                        )
                    })
                })
        }
        _ => None,
    }
}

fn type_reference_for_symbol(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                        return None;
                    };
                    (local.symbol == symbol).then_some(local.type_reference)
                })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned| owned.symbol == symbol)
                .map(|owned| owned.type_reference)
        })
        .or_else(|| {
            // State bindings are WHOLE-MACHINE scope: a param declared on the
            // machine's entry state is readable from every sub-state, so its
            // declared range must feed the prover there too (`pick(k: usize
            // [0..=3]) { ... state go { self.arr[k] } }`). Symbol match is
            // exact, so a sibling state's same-named param cannot alias.
            program.machine_states(machine).iter().find_map(|other| {
                program
                    .state_parameters(other)
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)
                    .map(|parameter| parameter.type_reference)
            })
        })
}

fn type_reference_for_name(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    name: &typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name == *name)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                        return None;
                    };
                    (local.name == *name).then_some(local.type_reference)
                })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned| owned.name == *name)
                .map(|owned| owned.type_reference)
        })
        .or_else(|| {
            // Whole-machine param scope (see the symbol twin above). A NAME
            // match is only sound when it is unambiguous machine-wide: two
            // states declaring same-named params with different ranges must
            // not prove against an arbitrary pick -- bail to unproven.
            let mut matches = program.machine_states(machine).iter().flat_map(|other| {
                program
                    .state_parameters(other)
                    .iter()
                    .filter(|parameter| parameter.name == *name)
            });
            let first = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            Some(first.type_reference)
        })
}

fn type_reference_is_slice(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
    member_symbol: SymbolHandle,
    member_name: &typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => data_field_type_reference(program, *referee, member_symbol, member_name),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => data_definition_by_symbol_or_name(program, *base_symbol, base_name).and_then(
            |data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            },
        ),
        TypeReferenceNode::Named { symbol, name } => {
            data_definition_by_symbol_or_name(program, *symbol, name).and_then(|data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            })
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn data_definition_by_symbol_or_name<'program>(
    program: &'program typed_trees::TypedTrees,
    symbol: SymbolHandle,
    name: &typed_trees::name::Identifier,
) -> Option<&'program typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|data_definition| {
        (symbol.is_valid() && data_definition.symbol == symbol) || data_definition.name == *name
    })
}

fn data_field_in_definition(
    program: &typed_trees::TypedTrees,
    data_definition: &typed_trees::data::DataDefinition,
    member_symbol: SymbolHandle,
    member_name: &typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(data_definition)
        .iter()
        .find_map(|member| {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };

            ((member_symbol.is_valid() && field.symbol == member_symbol)
                || field.name == *member_name)
                .then_some(field.type_reference)
        })
}
