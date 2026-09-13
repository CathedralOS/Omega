//! Structural parameters, projected shared operands, and primitive referents
//! in scalar computations. Parameter projections reuse established storage;
//! local construction remains independently restricted to supported whole values.

use super::*;

#[cfg(test)]
mod tests;

pub(crate) fn structural_computation_argument(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    target: &StateParameter,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if target.is_const {
        return None;
    }
    let target_access = structural_access_for_type_reference(program, target.type_reference)?;
    let named = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(argument) => {
            let TypeReferenceNode::Reference { access, .. } = program
                .type_reference_table
                .type_reference(target.type_reference)
            else {
                return None;
            };
            if argument.access != *access {
                return None;
            }
            argument.target
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => expression,
        _ => return None,
    };
    let mut place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        call.statement_index,
        expression,
    )?;
    crate::flow::normalize_attached_place_root(program, machine, state.symbol, &mut place);
    rejoin_computation_accesses(program, borrow, machine, state.symbol, call)?;
    let target_state = crate::find_state(program, call.target_symbol)?;
    let target_position = program
        .state_parameters(target_state)
        .iter()
        .position(|parameter| parameter == target)?;
    let ExpressionNode::Call(authored) = program
        .expression_table
        .expression(call.authored_expression)
    else {
        return None;
    };
    let target_parameters = program.state_parameters(target_state);
    let receiver_count = target_parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .count();
    let exact_actual = if target.is_self {
        receiver_count == 1 && target_position == 0 && authored.receiver == expression
    } else {
        program
            .expression_table
            .expression_handles(authored.arguments)
            .get(target_position.checked_sub(receiver_count)?)
            == Some(&expression)
    };
    if authored.target_symbol != call.target_symbol || !exact_actual {
        return None;
    }
    if target_access == CheckedStructuralAccess::SharedBorrow
        && let Some(argument) =
            shared_nominal_argument(program, borrow, machine, state, call, &place, target)
    {
        return Some(argument);
    }
    if target.is_self {
        return None;
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(named) else {
        return None;
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
        || place.root != facts::PlaceRoot::Symbol(name.symbol)
        || !place.segments.is_empty()
    {
        return None;
    }
    if target_access == CheckedStructuralAccess::Owned {
        if named != expression {
            return None;
        }
        // This retains authored actual identity. Graph publication rejoins
        // affine transfers after multiplicity checking produces permissions.
        return owned_parameter_argument(program, state, name.symbol, target).or_else(|| {
            owned_array_local_argument(program, state, call.statement_index, name.symbol, target)
        });
    }
    let target_type = plain_primitive_referent(program, target.type_reference)?;
    if super::super::primitive_store::primitive_local_before(
        program,
        state,
        call.statement_index,
        name.symbol,
    )
    .is_some()
    {
        return primitive_local_argument(
            program,
            borrow,
            machine,
            state,
            call,
            &place,
            target.type_reference,
        );
    }
    let parameters = program.state_parameters(state);
    let source_position = parameters
        .iter()
        .position(|source| source.symbol == name.symbol)?;
    let source = &parameters[source_position];
    if source.is_self
        || source.is_const
        || plain_primitive_referent(program, source.type_reference)? != target_type
    {
        return None;
    }
    let source_access = structural_access_for_type_reference(program, source.type_reference)?;
    if !matches!(
        (source_access, target_access),
        (
            CheckedStructuralAccess::SharedBorrow,
            CheckedStructuralAccess::SharedBorrow
        ) | (
            CheckedStructuralAccess::MutableBorrow,
            CheckedStructuralAccess::SharedBorrow
                | CheckedStructuralAccess::MutableBorrow
                | CheckedStructuralAccess::WriteOnlyBorrow
        ) | (
            CheckedStructuralAccess::WriteOnlyBorrow,
            CheckedStructuralAccess::WriteOnlyBorrow
        )
    ) {
        return None;
    }
    let access = exact_structural_borrow_access(
        program,
        borrow,
        machine,
        state.symbol,
        call,
        &place,
        target_access,
        &[],
    )?;
    if access != target_access {
        return None;
    }
    let parameter_index = parameters[..source_position]
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_none()
        })
        .count();
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(parameter_index).ok()?,
        },
        path: Vec::new(),
        type_identity: base_type_identity(program, target.type_reference, &[])?,
        access,
    })
}

/// A readable nominal argument borrows its existing parameter or local result
/// home. Initializer spelling does not select custody; publication independently
/// rejoins this symbol to its dominating structural establishment.
fn shared_nominal_argument(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    place: &crate::flow::CanonicalPlace,
    target: &StateParameter,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    if !place
        .segments
        .iter()
        .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. }))
    {
        return None;
    }
    // A shared formal borrows the selected record at its existing root/path.
    // Receiver syntax does not confer additional projection authority; both
    // lanes rejoin exact source type and the captured access occurrence below.
    let TypeReferenceNode::Reference {
        access: language_semantics::ReferenceAccess::Shared,
        referee,
        ..
    } = program
        .type_reference_table
        .type_reference(target.type_reference)
    else {
        return None;
    };
    if !matches!(
        program.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Named { .. }
    ) {
        return None;
    }
    let target_identity = if target.is_self {
        let owner = program.machines().iter().find(|owner| {
            program
                .machine_states(owner)
                .first()
                .is_some_and(|entry| entry.symbol == call.target_symbol)
        })?;
        let reference = program
            .type_reference_table
            .find_named_type_reference(owner.attached_data_symbol)?;
        base_type_identity(program, reference, &[])?
    } else {
        if !matches!(
            program.type_reference_table.type_reference(*referee),
            TypeReferenceNode::Named { .. }
        ) || !validation::has_plain_owned_contents_with_numeric_constraints(program, *referee)
        {
            return None;
        }
        base_type_identity(program, *referee, &[])?
    };
    let parameters = program.state_parameters(state);
    let (reference, source) = if let Some(position) = parameters.iter().position(|parameter| {
        parameter.symbol == symbol || (parameter.is_self && symbol == machine)
    }) {
        let parameter = &parameters[position];
        if parameter.is_const {
            return None;
        }
        let mut reference = match program
            .type_reference_table
            .type_reference(parameter.type_reference)
        {
            TypeReferenceNode::Reference {
                access:
                    language_semantics::ReferenceAccess::Shared
                    | language_semantics::ReferenceAccess::Mutable,
                referee,
                ..
            } => *referee,
            TypeReferenceNode::Named { .. } => parameter.type_reference,
            _ => return None,
        };
        if parameter.is_self {
            let owner = program
                .machines()
                .iter()
                .find(|owner| owner.symbol == machine)?;
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return None;
            };
            if *symbol != owner.symbol && *symbol != owner.attached_data_symbol {
                return None;
            }
            reference = program
                .type_reference_table
                .find_named_type_reference(owner.attached_data_symbol)?;
        }
        let ordinal = parameters[..position]
            .iter()
            .filter(|parameter| {
                program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
            })
            .count();
        (
            reference,
            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(ordinal).ok()?,
            },
        )
    } else {
        // A projection borrows the same established local home. Resolve its
        // endpoint below while retaining the declaring local as the owner.
        let mut locals = program
            .statement_table
            .statements(state.statement_nodes)
            .get(..call.statement_index)?
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
                _ => None,
            });
        let local = locals.next()?;
        // A shared invocation observes the current local home; mutability of
        // that home does not turn this call-local loan into a copied snapshot.
        if locals.next().is_some() || !local.initial_value.is_valid() {
            return None;
        }
        (
            local.type_reference,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
        )
    };
    if !matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Named { .. }
    ) || !validation::has_plain_owned_contents_with_numeric_constraints(program, reference)
        || !matches!(
            program.type_multiplicity(reference),
            Multiplicity::Affine | Multiplicity::Unrestricted
        )
    {
        return None;
    }
    let (reference, path) = if place.segments.is_empty() {
        (reference, Vec::new())
    } else {
        projected_argument_path(program, state.symbol, call.statement_index, place)?
    };
    let mut shapes = ShapeCollector::new(program);
    let identity = shapes.add_type(reference, &[], &[])?;
    // Whole scalar sums use the same established-place observation as records.
    // No payload is extracted, copied, or reconstructed to borrow the sum; its
    // source declaration and dominating call result remain separate evidence.
    // The existing case call channel observes whole copy scalar sums. Nested
    // sums require a separate projected payload/custody channel, not this loan.
    let whole_scalar_sum = path.is_empty() && shapes.types.len() == 1
        && program.type_multiplicity(reference) == Multiplicity::Unrestricted
        && shapes.types.get(&identity).is_some_and(|shape| {
            matches!(&shape.shape, CheckedUnitStructuralTypeShape::Sum { cases }
                if !cases.is_empty() && cases.iter().all(|case| case.fields.iter().all(|field|
                    !field.relevance.is_erased() && matches!(field.field_type, CheckedUnitStructuralFieldType::Scalar(_)))))
        });
    if identity != target_identity
        || !parameter_qualifications(program, &mut shapes, reference, &[])?.is_empty()
        || (!whole_scalar_sum
            && !shapes.types.values().all(|shape| {
                matches!(&shape.shape, CheckedUnitStructuralTypeShape::Record { fields }
                if fields.iter().all(|field| !field.relevance.is_erased()
                    && matches!(field.field_type, CheckedUnitStructuralFieldType::Scalar(_)
                        | CheckedUnitStructuralFieldType::Structural { .. })))
            }))
    {
        return None;
    }
    if target.is_self {
        let site = crate::find_call_site(
            program,
            machine,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let mut receiver = crate::flow::canonical_receiver_place_for_call_site(
            program,
            machine,
            state.symbol,
            &site,
        )?;
        crate::flow::normalize_attached_place_root(program, machine, state.symbol, &mut receiver);
        // Captured self retains the machine namespace; contextual expression
        // resolution uses its actual formal. Normalize only that exact pair,
        // leaving every projected field and unrelated root unchanged.
        if receiver.root == facts::PlaceRoot::Symbol(machine) {
            receiver.root = facts::PlaceRoot::Symbol(
                parameters
                    .iter()
                    .find(|parameter| parameter.is_self)?
                    .symbol,
            );
        }
        let expected_root = if place.root == facts::PlaceRoot::Symbol(machine) {
            facts::PlaceRoot::Symbol(
                parameters
                    .iter()
                    .find(|parameter| parameter.is_self)?
                    .symbol,
            )
        } else {
            place.root
        };
        if !call.has_receiver
            || receiver.root != expected_root
            || receiver.segments != place.segments
        {
            return None;
        }
        // The receiver is retained separately from the explicit access roster.
        // Its shared loan cannot overlap an exclusive explicit actual.
        if borrow
            .argument_accesses
            .span(call.accesses)?
            .iter()
            .any(|access| access.root_symbol == symbol && access.kind.is_exclusive())
        {
            return None;
        }
    } else if exact_structural_borrow_access(
        program,
        borrow,
        machine,
        state.symbol,
        call,
        place,
        CheckedStructuralAccess::SharedBorrow,
        &[],
    )? != CheckedStructuralAccess::SharedBorrow
    {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source,
        path,
        type_identity: identity,
        access: CheckedStructuralAccess::SharedBorrow,
    })
}

fn owned_array_local_argument(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    source_symbol: SymbolHandle,
    target: &StateParameter,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    if target.is_mutable
        || program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == source_symbol)
    {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut declarations = statements
        .get(..statement_index)?
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == source_symbol => Some(local),
            _ => None,
        });
    let local = declarations.next()?;
    if declarations.next().is_some()
        || local.is_mutable
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
        || !validation::is_closed_primitive_array_type(program, local.type_reference)
        || !validation::is_closed_primitive_array_type(program, target.type_reference)
        || crate::checks::type_multiplicity(program, local.type_reference)
            != Multiplicity::Unrestricted
        || crate::checks::type_multiplicity(program, target.type_reference)
            != Multiplicity::Unrestricted
    {
        return None;
    }
    let mut shapes = ShapeCollector::new(program);
    let type_identity = shapes.add_type(local.type_reference, &[], &[])?;
    if type_identity != shapes.add_type(target.type_reference, &[], &[])? {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: source_symbol,
        },
        path: Vec::new(),
        type_identity,
        access: CheckedStructuralAccess::Owned,
    })
}

fn owned_parameter_argument(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    source_symbol: SymbolHandle,
    target: &StateParameter,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let parameters = program.state_parameters(state);
    let source_position = parameters
        .iter()
        .position(|parameter| parameter.symbol == source_symbol)?;
    let source = &parameters[source_position];
    let mut shapes = ShapeCollector::new(program);
    for parameter in [source, target] {
        if parameter.is_self
            || parameter.is_const
            || parameter.is_mutable
            || !(matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Named { .. }
            ) || validation::is_closed_primitive_array_type(program, parameter.type_reference))
            || program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
            || !matches!(
                crate::checks::type_multiplicity(program, parameter.type_reference),
                Multiplicity::Unrestricted | Multiplicity::Affine
            )
            || !(validation::is_closed_primitive_array_type(program, parameter.type_reference)
                || validation::has_plain_owned_contents_with_numeric_constraints(
                    program,
                    parameter.type_reference,
                ))
            || structural_access_for_type_reference(program, parameter.type_reference)?
                != CheckedStructuralAccess::Owned
            || !parameter_qualifications(program, &mut shapes, parameter.type_reference, &[])?
                .is_empty()
        {
            return None;
        }
    }
    let type_identity = shapes.add_type(source.type_reference, &[], &[])?;
    if type_identity != shapes.add_type(target.type_reference, &[], &[])?
        || crate::checks::type_multiplicity(program, source.type_reference)
            != crate::checks::type_multiplicity(program, target.type_reference)
    {
        return None;
    }
    let parameter_index = parameters[..source_position]
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_none()
        })
        .count();
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(parameter_index).ok()?,
        },
        path: Vec::new(),
        type_identity,
        access: CheckedStructuralAccess::Owned,
    })
}

/// Borrow rows are an ordered observation roster, not a set of roots. Rebuild
/// with the existing collector so scalar reads occupy their actual positions.
fn rejoin_computation_accesses(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    source: &checked_trees::FlowCallFact,
) -> Option<()> {
    let mut states = borrow
        .states
        .iter()
        .map(|(_, row)| row)
        .filter(|row| row.machine_symbol == machine && row.state_symbol == state);
    let state_row = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let mut calls = borrow.calls.span(state_row.calls)?.iter().filter(|call| {
        call.statement_index == source.statement_index && call.call_ordinal == source.call_ordinal
    });
    let call = calls.next()?;
    if calls.next().is_some()
        || call.target_symbol != source.target_symbol
        || call.accesses != source.accesses
        || call.has_receiver != source.has_receiver
        || call.receiver_symbol != source.receiver_symbol
        || !program
            .expression_table
            .expression_is_valid(source.authored_expression)
    {
        return None;
    }
    let ExpressionNode::Call(authored) = program
        .expression_table
        .expression(source.authored_expression)
    else {
        return None;
    };
    let arguments = program
        .expression_table
        .expression_handles(authored.arguments);
    if arguments.len() != authored.arguments.count() as usize {
        return None;
    }
    let mut expected_segments = arena::Arena::new();
    let mut expected_accesses = arena::Arena::new();
    let expected = crate::borrow::accesses::collect_call_argument_accesses(
        program,
        &mut expected_segments,
        &mut expected_accesses,
        arguments,
        state,
        source.statement_index,
        machine,
    );
    let expected = expected_accesses.span(expected)?;
    let actual = borrow.argument_accesses.span(call.accesses)?;
    if expected.len() != actual.len() {
        return None;
    }
    for (expected, actual) in expected.iter().zip(actual) {
        if expected.root_symbol != actual.root_symbol
            || expected.kind != actual.kind
            || expected_segments.span(expected.segments)?
                != borrow.access_segments.span(actual.segments)?
        {
            return None;
        }
        if actual.kind.is_exclusive()
            && borrow
                .argument_accesses
                .span(call.accesses)?
                .iter()
                .filter(|candidate| candidate.root_symbol == actual.root_symbol)
                .count()
                != 1
        {
            return None;
        }
    }
    Some(())
}

fn plain_primitive_referent(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if !matches!(
        program.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Named { .. }
    ) {
        return None;
    }
    program.primitive_type_reference(*referee)
}

pub(super) fn primitive_local_argument(
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    place: &crate::flow::CanonicalPlace,
    target_type: TypeReferenceHandle,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let local = super::super::primitive_store::primitive_local_before(
        program,
        state,
        call.statement_index,
        symbol,
    )?;
    let target_access = structural_access_for_type_reference(program, target_type)?;
    let access = exact_structural_borrow_access(
        program,
        borrow,
        machine,
        state.symbol,
        call,
        place,
        target_access,
        &[],
    )?;
    let type_identity = base_type_identity(program, target_type, &[])?;
    if !place.segments.is_empty()
        || target_access == CheckedStructuralAccess::Owned
        || access != target_access
        || base_type_identity(program, local.type_reference, &[])? != type_identity
    {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol },
        path: Vec::new(),
        type_identity,
        access,
    })
}
