//! Reference values join ordinary result bindings to exact source loan events.
//! Record construction, projected use and cleanup replay the same authored
//! origins, current loan premises and weakening boundaries independently.

use checked_trees::{CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralResultBindingPlan};
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

// An owned formal has a compact type DAG but its returned-origin roster is
// materialized by path. Bound that expansion before semantic classification or
// allocation. These are private production limits, not reference semantics;
// the leaf limit matches the executable ingress roster's current capacity.
const MAX_REFERENCE_ORIGIN_LEAVES: usize = 4096;
const MAX_REFERENCE_ORIGIN_VISITS: usize = MAX_REFERENCE_ORIGIN_LEAVES * 2;
const MAX_REFERENCE_ORIGIN_DEPTH: usize = 128;

fn reference_origin_expansion_is_bounded(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    fn visit(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        depth: usize,
        visits: &mut usize,
        leaves: &mut usize,
    ) -> Option<()> {
        *visits = visits.checked_add(1)?;
        if *visits > MAX_REFERENCE_ORIGIN_VISITS || depth > MAX_REFERENCE_ORIGIN_DEPTH {
            return None;
        }
        let symbol = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } => {
                *leaves = leaves.checked_add(1)?;
                return (*leaves <= MAX_REFERENCE_ORIGIN_LEAVES).then_some(());
            }
            TypeReferenceNode::Named { symbol, .. } => *symbol,
            TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
            TypeReferenceNode::Constrained { base_type, .. } => {
                return visit(program, *base_type, depth + 1, visits, leaves);
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                return visit(program, *element_type, depth + 1, visits, leaves);
            }
            _ => return Some(()),
        };
        if let Some(data) =
            crate::program_scope::data_definitions::data_definition_by_symbol(program, symbol)
        {
            for member in program.data_members(data) {
                if let typed_trees::data::DataMember::Field(field) = member {
                    visit(program, field.type_reference, depth + 1, visits, leaves)?;
                }
            }
        }
        Some(())
    }
    visit(program, reference, 0, &mut 0, &mut 0).is_some()
}

/// Closed record construction can contain real mutable-reference carriers.
/// This classifier grants neither an initializer origin nor a loan lifetime.
pub fn is_reference_record(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    if !reference_origin_expansion_is_bounded(program, reference) {
        return false;
    }
    fn visit(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        seen: &mut Vec<SymbolHandle>,
    ) -> Option<bool> {
        if parts(program, reference).is_some()
            || shared_borrowed_parts(program, reference).is_some()
        {
            return Some(true);
        }
        if crate::has_plain_owned_contents_with_numeric_constraints(program, reference) {
            return Some(false);
        }
        let symbol = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => symbol,
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if arguments.count() == 0 => base_symbol,
            _ => return None,
        };
        if seen.contains(symbol) {
            return None;
        }
        let data =
            crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || program.type_multiplicity(reference) != Multiplicity::Affine
            || !program.data_type_parameters(data).is_empty()
            || program.machines().iter().any(|machine| {
                machine.attached_data_symbol == *symbol && machine.name.as_str().ends_with("::drop")
            })
        {
            return None;
        }
        seen.push(*symbol);
        let mut contains_reference = false;
        for member in program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            if field.relevance.is_erased() {
                return None;
            }
            contains_reference |= visit(program, field.type_reference, seen)?;
        }
        seen.pop();
        Some(contains_reference)
    }
    parts(program, reference).is_none() && visit(program, reference, &mut Vec::new()) == Some(true)
}

/// Reconstruct a leaf's exact formal ingress. Structural parameter ordinals
/// exclude scalar and const parameters, just like ordinary call preparation.
pub fn initializer_source(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let (referent, access) =
        parts(program, reference).or_else(|| shared_borrowed_parts(program, reference))?;
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(_)
    ) {
        return borrowed_initializer_source(program, state, expression, reference);
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.symbol != path.head_symbol
        || program.symbols.get(path.symbol).parent != state.symbol
    {
        return None;
    }
    let (position, parameter) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == path.symbol)?;
    if parameter.is_self
        || parameter.name != *name
        || !reference_parameter_can_supply(program, parameter.type_reference, reference)
    {
        return None;
    }
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(position).ok()?,
        },
        path: Vec::new(),
        type_identity: program.normalized_type_identity(referent).into_string(),
        access,
    })
}

/// Returning an ingress loan can reduce access, never change its referent.
/// Lifetime validity is checked by the borrow checker; this reconstruction
/// retains the exact referent (including qualifications). An outer constraint
/// shell is not silently dropped to make an access conversion succeed.
fn reference_parameter_can_supply(
    program: &TypedTrees,
    source: TypeReferenceHandle,
    result: TypeReferenceHandle,
) -> bool {
    if program.normalized_type_identity(source) == program.normalized_type_identity(result) {
        return true;
    }
    let TypeReferenceNode::Reference {
        referee: source_referee,
        access: language_core::ReferenceAccess::Mutable,
        ..
    } = program.type_reference_table.type_reference(source)
    else {
        return false;
    };
    let TypeReferenceNode::Reference {
        referee: result_referee,
        access: language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::WriteOnly,
        ..
    } = program.type_reference_table.type_reference(result)
    else {
        return false;
    };
    program.normalized_type_identity(*source_referee)
        == program.normalized_type_identity(*result_referee)
}

/// A borrowed initializer may name one projected leaf of a formal carrier —
/// `&receiver.field[i]` — where the whole-carrier arm above only spells a
/// bare parameter. The authored address walks member hops and literal index
/// hops off the receiver; the minted source is that parameter's position and
/// the projected segments. A final `Referent` step belongs only to a stored
/// reference leaf, never to an ordinary value being borrowed. Indexes bound
/// to locals stay opaque: folding them would need the
/// constant-binding evidence the producer resolves elsewhere.
pub fn borrowed_initializer_source(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let (referent, access) =
        parts(program, reference).or_else(|| shared_borrowed_parts(program, reference))?;
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
        return None;
    };
    let authored_access = match borrow.access {
        language_core::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
        language_core::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        language_core::ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
    };
    if authored_access != access {
        return None;
    }
    let mut projections = Vec::new();
    let mut cursor = borrow.target;
    loop {
        if !program.expression_table.expression_is_valid(cursor)
            || projections.contains(&cursor)
            || projections.len() >= MAX_REFERENCE_ORIGIN_DEPTH
        {
            return None;
        }
        cursor = match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                projections.push(cursor);
                member.receiver
            }
            ExpressionNode::Indexed(indexed) => {
                projections.push(cursor);
                indexed.collection
            }
            _ => break,
        };
    }
    if projections.is_empty() {
        return None;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(cursor) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if !path.symbol.is_valid() || path.symbol != path.head_symbol {
        return None;
    }
    let found = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == path.symbol);
    let (position, parameter) = found?;
    if parameter.is_self || parameter.name != *name {
        return None;
    }
    // Each member hop resolves against the nominal owner its receiver
    // projects: the root carrier's declaration first, then each selected
    // field's (or indexed element's) type.
    let mut owner_type = parameter.type_reference;
    let mut segments = Vec::with_capacity(projections.len() + 1);
    for projection in projections.iter().rev() {
        match program.expression_table.expression(*projection) {
            ExpressionNode::Member(member) => {
                let data = nominal_member_owner_type(program, owner_type)?;
                let field = crate::value_custody::places::exact_data_member_field(
                    program,
                    data,
                    member.member_symbol,
                    member.member.as_str(),
                    None,
                )?;
                if field.relevance.is_erased() {
                    return None;
                }
                segments.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    field.path_identity(),
                ));
                owner_type = field.type_reference;
            }
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) =
                    program.expression_table.expression(indexed.index)
                else {
                    return None;
                };
                segments.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(index.value_i64()?).ok()?,
                ));
                owner_type = loop {
                    match program.type_reference_table.type_reference(owner_type) {
                        TypeReferenceNode::Constrained { base_type, .. } => owner_type = *base_type,
                        TypeReferenceNode::FixedArray { element_type, .. } => break *element_type,
                        _ => return None,
                    }
                };
            }
            _ => return None,
        }
    }
    // Reference access and reference storage are separate facts. Borrowing a
    // scalar produces a reference but does not dereference the scalar itself.
    let leaf = if let Some(shell) = crate::reference_shell(program, owner_type) {
        if shell.access != borrow.access {
            return None;
        }
        segments.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
        shell.referee
    } else {
        owner_type
    };
    if program.normalized_type_identity(leaf) != program.normalized_type_identity(referent) {
        return None;
    }
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(position).ok()?,
        },
        path: segments,
        type_identity: program.normalized_type_identity(referent).into_string(),
        access,
    })
}

/// Peel constraint shells and reference wrappers to the nominal owner one
/// carrier's members live under.
fn nominal_member_owner_type<'p>(
    program: &'p TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<&'p typed_trees::data::DataDefinition> {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Named { symbol, .. } => {
                return crate::program_scope::data_definitions::data_definition_by_symbol(
                    program, *symbol,
                );
            }
            _ => return None,
        }
    }
}

/// Reconstruct the declaration-ordered reference roster from a complete
/// constructor. Reference leaves retain formal ingress instead of scalar data.
pub fn construction_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    construction_sources_with_calls(
        program,
        state,
        expression,
        reference,
        &mut vec![state.symbol],
    )
}

/// Completion and call substitution share one returned-leaf origin query.
/// Earlier ordinary statements do not change which authored value is returned.
pub fn returned_record_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    if !is_reference_record(program, state.return_type) {
        return None;
    }
    let expression = tail_return_expression(program, state)?;
    construction_sources(program, state, expression, state.return_type)
}

/// The expression producing a state's returned value: the tail expression
/// statement, or the exit arm of a terminal return transition.
pub fn tail_return_expression(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let last = program
        .statement_table
        .statements(state.statement_nodes)
        .last()?;
    match last {
        StatementNode::Expression(expression) => Some(*expression),
        StatementNode::Transition(transition)
            if matches!(
                transition.guard,
                typed_trees::statement::TransitionGuardNode::Always
            ) && transition.exit == typed_trees::statement::TransitionExit::Ordinary
                && !transition.continuation.is_valid() =>
        {
            match program.statement_table.transition_target(transition.target) {
                typed_trees::statement::TransitionTargetNode::Value(expression) => {
                    Some(*expression)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn construction_sources_with_calls(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    if parts(program, reference).is_some() || shared_borrowed_parts(program, reference).is_some() {
        return Some(vec![checked_trees::CheckedReferenceResultSourcePlan {
            path: Vec::new(),
            source: initializer_source(program, state, expression, reference)?,
        }]);
    }
    if crate::has_plain_owned_contents_with_numeric_constraints(program, reference) {
        return Some(Vec::new());
    }
    if !is_reference_record(program, reference) {
        return None;
    }
    if let ExpressionNode::Name(name) = program.expression_table.expression(expression) {
        let [spelling] = program.expression_table.name_path_members(name.members) else {
            return None;
        };
        if let Some((_, local)) = program.state_local_declaration(state, name.symbol) {
            if local.is_mutable
                || name.head_symbol != name.symbol
                || local.name != *spelling
                || program.symbols.get(name.symbol).parent != state.symbol
                || program.normalized_type_identity(local.type_reference)
                    != program.normalized_type_identity(reference)
                || active_origins.contains(&local.symbol)
            {
                return None;
            }
            active_origins.push(local.symbol);
            let sources = construction_sources_with_calls(
                program,
                state,
                local.initial_value,
                reference,
                active_origins,
            );
            active_origins.pop();
            return sources;
        }
        let (ordinal, parameter) = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == name.symbol)?;
        if parameter.is_self
            || name.head_symbol != name.symbol
            || program.symbols.get(name.symbol).parent != state.symbol
            || parameter.name != *spelling
            || program.normalized_type_identity(parameter.type_reference)
                != program.normalized_type_identity(reference)
        {
            return None;
        }
        return formal_record_sources(program, reference, u32::try_from(ordinal).ok()?);
    }
    if let ExpressionNode::Call(call) = program.expression_table.expression(expression) {
        let machine = program.machines().iter().find(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                && program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == call.target_symbol)
        })?;
        let [destination] = program.machine_states(machine) else {
            return None;
        };
        if active_origins.contains(&destination.symbol)
            || program.normalized_type_identity(destination.return_type)
                != program.normalized_type_identity(reference)
        {
            return None;
        }
        let parameters = program.state_parameters(destination);
        let arguments = program.expression_table.expression_handles(call.arguments);
        if parameters.len() != arguments.len()
            || parameters
                .iter()
                .any(|parameter| parameter.is_self || parameter.is_const)
        {
            return None;
        }
        let StatementNode::Expression(returned) = program
            .statement_table
            .statements(destination.statement_nodes)
            .last()?
        else {
            return None;
        };
        active_origins.push(destination.symbol);
        let sources = construction_sources_with_calls(
            program,
            destination,
            *returned,
            destination.return_type,
            active_origins,
        )?;
        active_origins.pop();
        sources
            .into_iter()
            .map(|mut source| {
                let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                } = source.source.source
                else {
                    return None;
                };
                let (position, parameter) = parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| {
                        program
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                    })
                    .nth(parameter_index as usize)?;
                source.source = if source.source.path.is_empty() {
                    initializer_source(
                        program,
                        state,
                        *arguments.get(position)?,
                        parameter.type_reference,
                    )?
                } else {
                    let (last, path) = source.source.path.split_last()?;
                    if *last != checked_trees::CheckedUnitStructuralPathSegment::Referent
                        || !is_reference_record(program, parameter.type_reference)
                    {
                        return None;
                    }
                    let actual = construction_sources_with_calls(
                        program,
                        state,
                        *arguments.get(position)?,
                        parameter.type_reference,
                        active_origins,
                    )?;
                    let mut matching = actual.iter().filter(|actual| actual.path == path);
                    let selected = matching.next()?;
                    if matching.next().is_some() {
                        return None;
                    }
                    selected.source.clone()
                };
                Some(source)
            })
            .collect()
    } else {
        record_construction_sources(program, state, expression, reference, active_origins)
    }
}

/// An owned formal transports its existing reference leaves. The source path
/// names each borrowed referent; it does not authorize creating a fresh loan
/// or moving storage through the referent boundary.
fn formal_record_sources(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    parameter_index: u32,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    fn visit(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        parameter_index: u32,
        path: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        output: &mut Vec<checked_trees::CheckedReferenceResultSourcePlan>,
    ) -> Option<()> {
        if let Some((referent, access)) =
            parts(program, reference).or_else(|| shared_borrowed_parts(program, reference))
        {
            let mut source_path = path.clone();
            source_path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
            output.push(checked_trees::CheckedReferenceResultSourcePlan {
                path: path.clone(),
                source: checked_trees::CheckedUnitStructuralArgumentPlan {
                    source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index,
                    },
                    path: source_path,
                    type_identity: program.normalized_type_identity(referent).into_string(),
                    access,
                },
            });
            return Some(());
        }
        if crate::has_plain_owned_contents_with_numeric_constraints(program, reference) {
            return Some(());
        }
        let symbol = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => symbol,
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if arguments.count() == 0 => base_symbol,
            _ => return None,
        };
        let data =
            crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
        for member in program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                field.path_identity(),
            ));
            visit(program, field.type_reference, parameter_index, path, output)?;
            path.pop();
        }
        Some(())
    }
    if !is_reference_record(program, reference) {
        return None;
    }
    let mut output = Vec::new();
    visit(
        program,
        reference,
        parameter_index,
        &mut Vec::new(),
        &mut output,
    )?;
    Some(output)
}

fn record_construction_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    let symbol = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => symbol,
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if arguments.count() == 0 => base_symbol,
        _ => return None,
    };
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        return None;
    };
    if literal.type_symbol != *symbol || literal.case_symbol.is_some() {
        return None;
    }
    let data = crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
    let initializers = program.expression_table.struct_fields(literal.fields);
    let fields = program.data_members(data);
    if initializers.len() != fields.len() {
        return None;
    }
    let mut output = Vec::new();
    for member in fields {
        let typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        let mut matching = initializers
            .iter()
            .filter(|initializer| initializer.field_symbol == field.symbol);
        let initializer = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        for mut source in construction_sources_with_calls(
            program,
            state,
            initializer.value,
            field.type_reference,
            active_origins,
        )? {
            source.path.insert(
                0,
                checked_trees::CheckedUnitStructuralPathSegment::Field(field.path_identity()),
            );
            output.push(source);
        }
    }
    Some(output)
}

/// Whether an immutable local of type `reference` must join its stored
/// carrier leaves to captured loans (`local_record_loans`). Record loans
/// belong to a locally constructed record holding reference carriers. A bare
/// `&'a V` shared view is no construction: a selected borrow binds it through
/// the shared-borrow join. `is_reference_record` counts that view for result
/// custody, so this local-construction gate excludes it explicitly. The
/// checked producer and the lowering replay both ask this one question.
pub fn local_owes_record_loans(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    is_reference_record(program, reference) && shared_borrowed_parts(program, reference).is_none()
}

/// Join each stored carrier leaf to its exact captured loan and weakening.
/// Similar lifetime spelling and UnretainedDerived alone are not authority.
pub fn local_record_loans(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
) -> Option<
    Vec<(
        checked_trees::CheckedReferenceResultSourcePlan,
        arena::Handle<checked_trees::BorrowLoanFact>,
    )>,
> {
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index as usize)?
    else {
        return None;
    };
    if local.is_mutable || !is_reference_record(program, local.type_reference) {
        return None;
    }
    let sources = construction_sources(program, state, local.initial_value, local.type_reference)?;
    let mut borrowing_states =
        facts
            .borrow
            .states
            .iter()
            .map(|(_, state)| state)
            .filter(|candidate| {
                candidate.machine_symbol == machine && candidate.state_symbol == state.symbol
            });
    let borrowing = borrowing_states.next()?;
    if borrowing_states.next().is_some() {
        return None;
    }
    let loans = facts
        .borrow
        .loans
        .iter()
        .filter(|(handle, loan)| {
            facts.borrow.state_owns_loan(borrowing, *handle) && loan.owner_symbol == local.symbol
        })
        .collect::<Vec<_>>();
    if sources.len() != loans.len() {
        return None;
    }
    let flow = state_flow(facts, machine, state.symbol)?;
    let mut output = Vec::new();
    for source in sources {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            source.source.source
        else {
            return None;
        };
        let parameter = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .nth(parameter_index as usize)?;
        let mut owner_path = Vec::new();
        let mut selected = local.type_reference;
        for segment in &source.path {
            let checked_trees::CheckedUnitStructuralPathSegment::Field(identity) = segment else {
                return None;
            };
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(selected)
            else {
                return None;
            };
            let data = crate::program_scope::data_definitions::data_definition_by_symbol(
                program, *symbol,
            )?;
            let field = program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field)
                        if field.path_identity() == *identity =>
                    {
                        Some(field)
                    }
                    _ => None,
                })?;
            owner_path.push(checked_trees::BorrowLoanOwnerSegment::Field(field.symbol));
            selected = field.type_reference;
        }
        let mut matching = loans
            .iter()
            .filter(|(_, loan)| facts.borrow.loan_owner_path(loan) == owner_path);
        let (handle, loan) = *matching.next()?;
        // A stored carrier's loan kind is the access its field declared: a
        // shared `&T` captures a read loan, exclusive carriers capture their
        // mutable or write-only loan -- the same kind derivation
        // `result_loan` applies to shared views.
        let expected_kind = match source.source.access {
            CheckedStructuralAccess::SharedBorrow => checked_trees::BorrowAccessKind::Read,
            CheckedStructuralAccess::MutableBorrow => checked_trees::BorrowAccessKind::Mutable,
            CheckedStructuralAccess::WriteOnlyBorrow => checked_trees::BorrowAccessKind::WriteOnly,
            CheckedStructuralAccess::Owned => return None,
        };
        let expected_segments =
            expected_loan_segments(program, parameter.type_reference, &source.source.path)?;
        if matching.next().is_some()
            || loan.statement_index != statement_index as usize
            || loan.root_symbol != parameter.symbol
            || loan.kind != expected_kind
            || facts.borrow.loan_segments(loan) != expected_segments.as_slice()
        {
            return None;
        }
        if let Some((owner, prior)) = moved_record_source(
            program,
            facts,
            machine,
            state,
            statement_index,
            &source.path,
        )? {
            let previous = facts.borrow.loans.get(prior);
            if loan.source_owner_symbol != owner
                || loan.root_symbol != previous.root_symbol
                || loan.kind != previous.kind
                || facts.borrow.loan_segments(loan) != facts.borrow.loan_segments(previous)
            {
                return None;
            }
        } else if loan.source_owner_symbol.is_valid() {
            return None;
        }
        let mut activations = facts
            .flow
            .borrow_lifetimes
            .activations
            .span_or_empty(flow.borrow_activations)
            .iter()
            .filter(|activation| activation.loan == handle);
        if activations.next()?.source
            != (checked_trees::FlowInvalidationSource::Statement {
                statement_index: statement_index as usize,
            })
            || activations.next().is_some()
        {
            return None;
        }
        release_boundary(program, facts, machine, state.symbol, handle)?;
        output.push((source, handle));
    }
    Some(output)
}

/// The loan segments a borrowed initializer source projects under its
/// parameter's declared type, spelled in the ownership vocabulary the
/// ledger records. Field hops resolve through the nominal owner exactly as
/// the owner-path walk does; literal fixed-array hops keep their ordinal.
/// Runtime-indexed, case and referent hops have no initializer-side
/// producer (`borrowed_initializer_source` admits Field and FixedIndex
/// hops only), so anything else declines the source.
fn expected_loan_segments(
    program: &TypedTrees,
    root: TypeReferenceHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<facts::PlaceSegment>> {
    let mut selected = root;
    let mut segments = Vec::with_capacity(path.len());
    for segment in path {
        match segment {
            checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => {
                let data = nominal_member_owner_type(program, selected)?;
                let field = program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Field(field)
                            if field.path_identity() == *identity =>
                        {
                            Some(field)
                        }
                        _ => None,
                    })?;
                segments.push(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
                selected = field.type_reference;
            }
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                let mut carrier = selected;
                loop {
                    match program.type_reference_table.type_reference(carrier) {
                        TypeReferenceNode::Constrained { base_type, .. } => carrier = *base_type,
                        TypeReferenceNode::FixedArray { element_type, .. }
                        | TypeReferenceNode::Slice { element_type } => {
                            selected = *element_type;
                            break;
                        }
                        _ => return None,
                    }
                }
                segments.push(facts::PlaceSegment::FixedIndex {
                    index: usize::try_from(*index).ok()?,
                });
            }
            _ => return None,
        }
    }
    Some(segments)
}

/// Replay an owned actual's captured leaf at the exact consuming call. This
/// establishes source correspondence, not a new reborrow lineage: the owned
/// carrier and its existing permission move together.
fn moved_record_source(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    result_path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Option<(SymbolHandle, arena::Handle<checked_trees::BorrowLoanFact>)>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(local) = statements.get(statement_index as usize)? else {
        return None;
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(local.initial_value)
    else {
        return Some(None);
    };
    let destination = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|candidate| candidate.symbol == call.target_symbol)?;
    let sources = returned_record_sources(program, destination)?;
    let source = sources.iter().find(|source| source.path == result_path)?;
    if source.source.path.is_empty() {
        return Some(None);
    }
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
        source.source.source
    else {
        return None;
    };
    let (position, parameter) = program
        .state_parameters(destination)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| {
            !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .nth(parameter_index as usize)?;
    if !is_reference_record(program, parameter.type_reference) {
        return None;
    }
    let actual = *program
        .expression_table
        .expression_handles(call.arguments)
        .get(position)?;
    let ExpressionNode::Name(name) = program.expression_table.expression(actual) else {
        return None;
    };
    if name.head_symbol != name.symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return None;
    }
    let (prior_index, prior_local) = program
        .state_local_declaration(state, name.symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index as usize)?;
    if program.normalized_type_identity(prior_local.type_reference)
        != program.normalized_type_identity(parameter.type_reference)
    {
        return None;
    }
    let (last, path) = source.source.path.split_last()?;
    if *last != checked_trees::CheckedUnitStructuralPathSegment::Referent {
        return None;
    }
    let prior = local_record_loans(
        program,
        facts,
        machine,
        state,
        u32::try_from(prior_index).ok()?,
    )?;
    let mut matching = prior.iter().filter(|(source, _)| source.path == path);
    let (_, handle) = matching.next()?;
    if matching.next().is_some()
        || !record_loan_is_active(
            program,
            facts,
            machine,
            state.symbol,
            statement_index,
            *handle,
        )
    {
        return None;
    }
    Some(Some((prior_local.symbol, *handle)))
}

/// Join one leaf of a call's returned-leaf roster through the call's exact
/// actual to a caller-side local's captured loan. This is the operand form of
/// `moved_record_source`: the producing call is named by its authored
/// expression rather than a `let` statement's initializer, and the consuming
/// statement's activity or last-use discipline is checked by the caller.
fn call_actual_leaf_loan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    call: &typed_trees::expression::TableCallExpression,
    destination: &typed_trees::state::State,
    source: &checked_trees::CheckedReferenceResultSourcePlan,
) -> Option<arena::Handle<checked_trees::BorrowLoanFact>> {
    let [
        path @ ..,
        checked_trees::CheckedUnitStructuralPathSegment::Referent,
    ] = source.source.path.as_slice()
    else {
        return None;
    };
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
        source.source.source
    else {
        return None;
    };
    let (position, parameter) = program
        .state_parameters(destination)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| {
            !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .nth(parameter_index as usize)?;
    if !is_reference_record(program, parameter.type_reference) {
        return None;
    }
    let actual = *program
        .expression_table
        .expression_handles(call.arguments)
        .get(position)?;
    let ExpressionNode::Name(name) = program.expression_table.expression(actual) else {
        return None;
    };
    if name.head_symbol != name.symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return None;
    }
    let (prior_index, prior_local) = program
        .state_local_declaration(state, name.symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index as usize)?;
    if program.normalized_type_identity(prior_local.type_reference)
        != program.normalized_type_identity(parameter.type_reference)
    {
        return None;
    }
    let prior = local_record_loans(
        program,
        facts,
        machine,
        state,
        u32::try_from(prior_index).ok()?,
    )?;
    let mut matching = prior.iter().filter(|(source, _)| source.path == path);
    let (_, handle) = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(*handle)
}

fn record_loan_is_active(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
    loan: arena::Handle<checked_trees::BorrowLoanFact>,
) -> bool {
    let Some(flow) = state_flow(facts, machine, state) else {
        return false;
    };
    let Some(entries) = facts.flow.control.statements.span(flow.statements) else {
        return false;
    };
    let mut entries = entries
        .iter()
        .filter(|entry| entry.statement_index == statement_index as usize);
    let Some(entry) = entries.next() else {
        return false;
    };
    entries.next().is_none()
        && facts.flow.contexts.constraint_refs.span(entry.entry_constraints).is_some_and(|constraints|
            constraints.iter().filter(|constraint| matches!(constraint.kind, checked_trees::FlowConstraintKind::BorrowLoan { loan: active } if active == loan)).count() == 1)
        && facts.borrow.loans.get(loan).statement_index < statement_index as usize
        && release_boundary(program, facts, machine, state, loan).is_some_and(|end| statement_index < end)
}

/// Whole owned operands retain every captured leaf until their consuming call.
pub fn owned_record_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    source_statement: u32,
) -> bool {
    source_statement < statement_index
        && local_record_loans(program, facts, machine, state, source_statement).is_some_and(
            |loans| {
                !loans.is_empty()
                    && loans.iter().all(|(_, loan)| {
                        record_loan_is_active(
                            program,
                            facts,
                            machine,
                            state.symbol,
                            statement_index,
                            *loan,
                        )
                    })
            },
        )
}

/// Resolve an authored member projection against a record type, yielding the
/// field-identity path and the projected type. The spelling matches the leaf
/// paths `formal_record_sources` and `local_record_loans` reconstruct, so one
/// resolver keeps those joins from drifting.
pub(crate) fn declared_field_path<'a>(
    program: &TypedTrees,
    mut selected: TypeReferenceHandle,
    members: impl Iterator<Item = &'a typed_trees::expression::TableMemberExpression>,
) -> Option<(
    Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    TypeReferenceHandle,
)> {
    let mut path = Vec::new();
    for member in members {
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(selected)
        else {
            return None;
        };
        let data =
            crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
        let field = crate::value_custody::places::exact_data_member_field(
            program,
            data,
            member.member_symbol,
            member.member.as_str(),
            member.case_variant.as_ref().map(|case| case.as_str()),
        )?;
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            field.path_identity(),
        ));
        selected = field.type_reference;
    }
    Some((path, selected))
}

/// A projected owned-record argument moves the projected subtree's captured
/// leaves with the call. Every leaf under the projected edge must stay live
/// until this consuming statement, and the edge must carry the record's whole
/// leaf roster so no sibling custody strands under a consumed residual shell.
/// `path` is the projected field-identity path exactly as
/// `projected_argument_path` spells it.
pub fn projected_record_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    source_statement: u32,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> bool {
    !path.is_empty()
        && source_statement < statement_index
        && local_record_loans(program, facts, machine, state, source_statement).is_some_and(
            |loans| {
                !loans.is_empty()
                    && loans.iter().all(|(source, loan)| {
                        source.path.starts_with(path)
                            && record_loan_is_active(
                                program,
                                facts,
                                machine,
                                state.symbol,
                                statement_index,
                                *loan,
                            )
                    })
            },
        )
}

/// A projected operand may root at a nested call's anonymous result instead
/// of a named local: `select(forward_outer(outer).inner)`. Every leaf the
/// nested result publishes under the projected edge must replay one
/// caller-side local's captured leaf loan through the call's exact actual,
/// and each such loan must stay live until the consuming call — the same
/// custody `projected_record_argument` proves for a stored carrier. The
/// anonymous carrier itself is consumed whole by the projection, so its
/// entire returned leaf roster must live under the edge.
pub fn nested_call_record_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    let Some(destination) = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|candidate| candidate.symbol == call.target_symbol)
    else {
        return false;
    };
    if !is_reference_record(program, destination.return_type) {
        return false;
    }
    let Some(sources) = returned_record_sources(program, destination) else {
        return false;
    };
    !path.is_empty()
        && !sources.is_empty()
        && sources.iter().all(|source| {
            source.path.starts_with(path)
                && call_actual_leaf_loan(
                    program,
                    facts,
                    machine,
                    state,
                    statement_index,
                    call,
                    destination,
                    source,
                )
                .is_some_and(|loan| {
                    record_loan_is_active(
                        program,
                        facts,
                        machine,
                        state.symbol,
                        statement_index,
                        loan,
                    )
                })
        })
}

/// A nested call's anonymous reference-record result has no `let` destination:
/// `select(forward_outer(outer).inner)` captures `forward_outer`'s returned
/// leaves directly into the consuming call. Each returned leaf must replay
/// through the call's exact actual to one caller-side local's captured loan
/// that stays live at the containing statement — the same leaf join
/// `nested_call_record_argument` proves under the consumer's projected edge,
/// here required of the complete returned roster.
pub fn nested_call_result_loans(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    let Some(destination) = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|candidate| candidate.symbol == call.target_symbol)
    else {
        return false;
    };
    if !is_reference_record(program, destination.return_type) {
        return false;
    }
    let Some(sources) = returned_record_sources(program, destination) else {
        return false;
    };
    !sources.is_empty()
        && sources.iter().all(|source| {
            call_actual_leaf_loan(
                program,
                facts,
                machine,
                state,
                statement_index,
                call,
                destination,
                source,
            )
            .is_some_and(|loan| {
                record_loan_is_active(program, facts, machine, state.symbol, statement_index, loan)
            })
        })
}

/// A projected argument crosses the stored carrier only after its exact local
/// construction and active leaf loan have been reconstructed.
pub fn record_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
    destination: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let (referent, access) = parts(program, destination)?;
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)?
    else {
        return None;
    };
    if result.type_identity
        != program
            .normalized_type_identity(local.type_reference)
            .as_str()
        || result.multiplicity != program.type_multiplicity(local.type_reference)
    {
        return None;
    }
    let mut members = Vec::new();
    let mut root = expression;
    while let ExpressionNode::Member(member) = program.expression_table.expression(root) {
        members.push(member);
        root = member.receiver;
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(root) else {
        return None;
    };
    if name.symbol != local.symbol
        || name.head_symbol != local.symbol
        || !matches!(program.expression_table.name_path_members(name.members), [name] if *name == local.name)
    {
        return None;
    }
    let (mut path, reference) =
        declared_field_path(program, local.type_reference, members.into_iter().rev())?;
    if program.normalized_type_identity(reference) != program.normalized_type_identity(destination)
    {
        return None;
    }
    let loans = local_record_loans(program, facts, machine, state, result.statement_index)?;
    let (source, loan) = loans.iter().find(|(source, _)| source.path == path)?;
    let flow = state_flow(facts, machine, state.symbol)?;
    let mut entries = facts
        .flow
        .control
        .statements
        .span(flow.statements)?
        .iter()
        .filter(|entry| entry.statement_index == statement_index as usize);
    let entry = entries.next()?;
    if entries.next().is_some()
        || facts.flow.contexts.constraint_refs.span(entry.entry_constraints)?.iter()
            .filter(|constraint| matches!(constraint.kind, checked_trees::FlowConstraintKind::BorrowLoan { loan: active } if active == *loan))
            .count() != 1
    {
        return None;
    }
    if statement_index <= result.statement_index
        || statement_index >= release_boundary(program, facts, machine, state.symbol, *loan)?
        || source.source.type_identity != program.normalized_type_identity(referent).as_str()
    {
        return None;
    }
    path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: source.source.type_identity.clone(),
        access,
    })
}

/// Stored reference classification precedes referent-oriented normalization.
/// The first executable carrier retains a mutable, unqualified primitive loan.
pub fn parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if *access != language_semantics::ReferenceAccess::Mutable
        || !matches!(
            program.type_reference_table.type_reference(*referee),
            TypeReferenceNode::Named { .. }
        )
        || program.primitive_type_reference(*referee).is_none()
    {
        return None;
    }
    Some((*referee, CheckedStructuralAccess::MutableBorrow))
}

/// The referee beneath any `&mut T` carrier, with constraint shells peeled.
/// Unlike `parts` this does not require a primitive referent: a
/// `&mut Record` names storage in the caller's frame just as a `&mut u8`
/// does. Callers that need the primitive-only reference-result shape keep
/// using `parts`.
pub fn mutable_borrowed_parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| shell.access == language_semantics::ReferenceAccess::Mutable)
        .map(|shell| (shell.referee, CheckedStructuralAccess::MutableBorrow))
}

/// The referee beneath a `&write T` carrier, with constraint shells peeled —
/// the write-only analog of `mutable_borrowed_parts`. A write-only loan names
/// the same caller-frame storage an exclusive borrow does; only the access
/// authority differs (writes admitted, reads fenced).
pub fn write_only_borrowed_parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| shell.access == language_semantics::ReferenceAccess::WriteOnly)
        .map(|shell| (shell.referee, CheckedStructuralAccess::WriteOnlyBorrow))
}

/// The named referee beneath a `&'a V` shared-borrowed view, with constraint
/// shells peeled. The referent's storage lives in the caller's frame like
/// `parts`' loan — it just cannot be written through. The referee must be
/// spelled as a `Named` type: this is the shared reference-result shape.
pub fn shared_borrowed_parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| {
            shell.access == language_semantics::ReferenceAccess::Shared
                && matches!(
                    program.type_reference_table.type_reference(shell.referee),
                    TypeReferenceNode::Named { .. }
                )
        })
        .map(|shell| (shell.referee, CheckedStructuralAccess::SharedBorrow))
}

/// The referee beneath any `& T` place-loan carrier, with constraint shells
/// peeled but without `shared_borrowed_parts`' named-referent restriction. A
/// `&[u8; 48]` loan names the array's storage in the caller's frame just as
/// a `&u32` does — the same coverage `mutable_borrowed_parts` gives the
/// exclusive side. Callers that need the shared reference-result shape keep
/// using `shared_borrowed_parts`.
pub fn shared_place_borrowed_parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| shell.access == language_semantics::ReferenceAccess::Shared)
        .map(|shell| (shell.referee, CheckedStructuralAccess::SharedBorrow))
}

/// Source reference use multiplicity does not encode the lifetime obligation
/// of a materialized carrier. Mutable carrier custody must end exactly once.
pub fn result_multiplicity(program: &TypedTrees, reference: TypeReferenceHandle) -> Multiplicity {
    if parts(program, reference).is_some()
        || shared_borrowed_parts(program, reference).is_some()
        || mutable_borrowed_parts(program, reference).is_some()
    {
        Multiplicity::Affine
    } else {
        program.type_multiplicity(reference)
    }
}

pub fn source_parameter(program: &TypedTrees, state: &typed_trees::state::State) -> Option<usize> {
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()?
    else {
        return None;
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(*expression) else {
        return None;
    };
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    // The same ingress reconstruction serves bare returns and stored reference
    // initializers. Parameter identity is explicit; attached-machine identity
    // cannot stand in for an authored argument.
    initializer_source(program, state, *expression, state.return_type)?;
    program
        .state_parameters(state)
        .iter()
        .position(|parameter| {
            parameter.symbol == path.symbol && !parameter.is_self && !parameter.is_const
        })
}

/// A bare reference result may also name one projected leaf of an owned
/// record formal: `value.body` selects the carrier leaf without minting a
/// fresh source. The returned position indexes the callee's authored
/// parameters (call arguments share that order); the argument plan is the
/// leaf's borrowed-referent ingress exactly as `formal_record_sources`
/// encodes it. Only declared `Member` projections through reference records
/// qualify — indexes, case payloads, and non-record receivers stay opaque.
pub fn source_leaf(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<(usize, checked_trees::CheckedUnitStructuralArgumentPlan)> {
    parts(program, state.return_type)?;
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()?
    else {
        return None;
    };
    let mut members = Vec::new();
    let mut current = *expression;
    while let ExpressionNode::Member(member) = program.expression_table.expression(current) {
        if member.case_variant.is_some() {
            return None;
        }
        members.push(member);
        current = member.receiver;
    }
    if members.is_empty() {
        return None;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(current) else {
        return None;
    };
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let (position, parameter) = program
        .state_parameters(state)
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == path.symbol)?;
    if parameter.is_self
        || parameter.is_const
        || program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
    {
        return None;
    }
    // Every projection must stay inside a checked reference record and the
    // selected leaf must be the declared reference result itself.
    let mut selected = parameter.type_reference;
    let mut leaf_path = Vec::new();
    for member in members.iter().rev() {
        if !is_reference_record(program, selected) {
            return None;
        }
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(selected)
        else {
            return None;
        };
        let data =
            crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
        let member_symbol = member.member_symbol;
        let field = program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == member_symbol => {
                    Some(field)
                }
                _ => None,
            })?;
        leaf_path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            field.path_identity(),
        ));
        selected = field.type_reference;
    }
    if program.normalized_type_identity(selected)
        != program.normalized_type_identity(state.return_type)
    {
        return None;
    }
    let (ordinal, _) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .enumerate()
        .find(|(_, candidate)| candidate.symbol == parameter.symbol)?;
    let sources = formal_record_sources(
        program,
        parameter.type_reference,
        u32::try_from(ordinal).ok()?,
    )?;
    // The establishment moves the carrier's leaf into the bare reference
    // result and consumes the carrier whole, so the selected leaf must be the
    // record's entire declared reference roster. A sibling leaf would strand
    // live custody under a consumed place.
    if sources.len() != 1 {
        return None;
    }
    sources
        .into_iter()
        .find(|source| source.path == leaf_path)
        .map(|source| (position, source.source))
}

/// The authored parameter position every `&mut` result exit borrows from.
/// Each returned place rejoins through member folds and exact `&mut` view
/// loans until it reaches one parameter or `*self`; all exits must agree on
/// that caller ingress. Their projections deliberately remain distinct: this
/// join authorizes the one conservative caller loan, not a chosen return path.
fn mutable_view_source_parameter(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
) -> Option<usize> {
    if mutable_borrowed_parts(program, state.return_type).is_none() {
        return None;
    }
    let Some(machine) = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
    }) else {
        return None;
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut source = None;
    for (ordinal, statement) in statements.iter().enumerate() {
        let returned = match statement {
            StatementNode::Transition(transition) => {
                match program.statement_table.transition_target(transition.target) {
                    typed_trees::statement::TransitionTargetNode::Value(expression) => {
                        Some(*expression)
                    }
                    _ => None,
                }
            }
            StatementNode::Expression(expression) if ordinal + 1 == statements.len() => {
                Some(*expression)
            }
            _ => None,
        };
        let Some(returned) = returned else {
            continue;
        };
        let next = mutable_view_exit_source_parameter(program, facts, machine, state, returned)?;
        if source.is_some_and(|prior| prior != next) {
            return None;
        }
        source = Some(next);
    }
    source
}

/// Resolve one returned projection to the ingress whose loan it descends
/// from. This answers only the authored parameter position; the return arm's
/// distinct field/index path remains in its own checked reference formation.
fn mutable_view_exit_source_parameter(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<usize> {
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
        return None;
    };
    if borrow.access != language_semantics::ReferenceAccess::Mutable {
        return None;
    }
    let authored = program.state_parameters(state);
    let mut cursor = borrow.target;
    let mut symbol = loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Name(name) => break name.symbol,
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
            _ => {
                return None;
            }
        }
    };
    for _ in 0..8 {
        if authored.iter().any(|parameter| parameter.symbol == symbol) || symbol == machine.symbol {
            break;
        }
        if view_self_member_symbol(program, state, machine.symbol, symbol).is_some() {
            symbol = machine.symbol;
            continue;
        }
        let Some(borrow_state) = facts
            .borrow
            .states
            .iter()
            .map(|(_, candidate)| candidate)
            .find(|candidate| {
                candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol
            })
        else {
            return None;
        };
        let mut loans = facts.borrow.loans.iter().filter(|(handle, loan)| {
            facts.borrow.state_owns_loan(borrow_state, *handle)
                && loan.owner_symbol == symbol
                && loan.kind == checked_trees::BorrowAccessKind::Mutable
                && facts.borrow.loan_owner_path(loan).is_empty()
        });
        let Some((_, loan)) = loans.next() else {
            return None;
        };
        if loans.next().is_some() {
            return None;
        }
        symbol = loan.root_symbol;
    }
    if authored.iter().all(|parameter| parameter.symbol != symbol) && symbol != machine.symbol {
        return None;
    }
    let position = if symbol == machine.symbol {
        authored.iter().position(|parameter| parameter.is_self)?
    } else {
        authored
            .iter()
            .position(|parameter| parameter.symbol == symbol)?
    };
    if !view_source_parameter_is_mutable(program, authored.get(position)?) {
        return None;
    }
    Some(position)
}

/// The authored parameter position a shared `&'a V` view return borrows from,
/// read off the callee's tail expression the same way
/// `mutable_view_source_parameter` does for `&mut` results: a place borrow
/// spelled `transition { _ -> &self.node }` walks back through member folds
/// and `&` view loans until it reaches a parameter or `*self`. The loans a
/// shared derivation rejoins through are `Read` loans — `&mut` storage may
/// weaken to a shared read, but the denoted root must itself be readable, so
/// a `&write` carrier never admits a shared result.
fn shared_view_source_parameter(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
) -> Option<usize> {
    if shared_borrowed_parts(program, state.return_type).is_none() {
        return None;
    }
    let expression = tail_return_expression(program, state)?;
    let Some(machine) = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
    }) else {
        return None;
    };
    let authored = program.state_parameters(state);
    // A tail `&self.node` spells the borrow explicitly; a bare `self`
    // tail is the same shared source through the `&self -> &T` coercion,
    // naming the receiver without a borrow expression at all.
    let mut symbol = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow)
            if borrow.access == language_semantics::ReferenceAccess::Shared =>
        {
            let mut cursor = borrow.target;
            loop {
                match program.expression_table.expression(cursor) {
                    ExpressionNode::Name(name) => break name.symbol,
                    ExpressionNode::Member(member) => cursor = member.receiver,
                    ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
                    _ => {
                        return None;
                    }
                }
            }
        }
        ExpressionNode::Name(name) => name.symbol,
        _ => {
            return None;
        }
    };
    for _ in 0..8 {
        if authored.iter().any(|parameter| parameter.symbol == symbol) || symbol == machine.symbol {
            break;
        }
        if view_self_member_symbol(program, state, machine.symbol, symbol).is_some() {
            symbol = machine.symbol;
            continue;
        }
        let Some(borrow_state) = facts
            .borrow
            .states
            .iter()
            .map(|(_, candidate)| candidate)
            .find(|candidate| {
                candidate.machine_symbol == machine.symbol && candidate.state_symbol == state.symbol
            })
        else {
            return None;
        };
        let mut loans = facts.borrow.loans.iter().filter(|(handle, loan)| {
            facts.borrow.state_owns_loan(borrow_state, *handle)
                && loan.owner_symbol == symbol
                && matches!(
                    loan.kind,
                    checked_trees::BorrowAccessKind::Read
                        | checked_trees::BorrowAccessKind::Mutable
                )
                && facts.borrow.loan_owner_path(loan).is_empty()
        });
        let Some((_, loan)) = loans.next() else {
            return None;
        };
        if loans.next().is_some() {
            return None;
        }
        symbol = loan.root_symbol;
    }
    if authored.iter().all(|parameter| parameter.symbol != symbol) && symbol != machine.symbol {
        return None;
    }
    let position = if symbol == machine.symbol {
        authored.iter().position(|parameter| parameter.is_self)?
    } else {
        authored
            .iter()
            .position(|parameter| parameter.symbol == symbol)?
    };
    if !view_source_parameter_is_readable(program, authored.get(position)?) {
        return None;
    }
    Some(position)
}

/// The callee-side root a shared `&` view result denotes: any parameter a
/// shared read can reborrow from — `&`/`&mut` referents and owned or mutable
/// storage. A write-only carrier fences reads, so it is the one authority a
/// shared result cannot borrow through.
fn view_source_parameter_is_readable(
    program: &TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    write_only_borrowed_parts(program, parameter.type_reference).is_none()
}

/// The callee-side root a `&mut` view result denotes: a `&mut` parameter
/// symbol, the machine `*self` authority, or a `self` member — the member only
/// while `self` is mutably borrowed.
fn view_source_parameter_is_mutable(
    program: &TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    parameter.is_mutable || mutable_borrowed_parts(program, parameter.type_reference).is_some()
}

/// The field symbol `root` names inside the `*self` receiver's attached data,
/// when `self` is mutably borrowed — the root a minted view loan publishes
/// for `self.<field>` storage.
fn view_self_member_symbol(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    machine: SymbolHandle,
    root: SymbolHandle,
) -> Option<SymbolHandle> {
    let parameters = program.state_parameters(state);
    let self_parameter = parameters.iter().find(|parameter| parameter.is_self)?;
    if !view_source_parameter_is_mutable(program, self_parameter) {
        return None;
    }
    let mut type_reference = self_parameter.type_reference;
    let mut symbol = loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Named { symbol, .. } => break *symbol,
            _ => return None,
        }
    };
    if symbol == machine {
        symbol = program.machine_by_symbol(machine)?.attached_data_symbol;
    }
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)?;
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.symbol == root => {
                Some(field.symbol)
            }
            _ => None,
        })
}

/// Reconstruct actual call substitution, not only the return lifetime annotation.
/// The callee's ordinary completion independently verifies this ingress source.
pub fn result_loan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<arena::Handle<checked_trees::BorrowLoanFact>> {
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)?
    else {
        return None;
    };
    // `mutable_borrowed_parts` peels every `&mut`, including the whole
    // primitive references `parts` already owns; those keep the ingress and
    // leaf rosters. Only a non-primitive `&mut` view takes the tail walk.
    // A shared `&T` reference result is the read-side member of the same
    // family: primitive `&u8` results are Unrestricted and reach custody
    // through `permits_disposal`, while a `&` record referent is Affine and
    // needs this tail walk exactly like a `&mut` view does.
    let mutable_view = parts(program, local.type_reference).is_none()
        && mutable_borrowed_parts(program, local.type_reference).is_some();
    let shared_view = parts(program, local.type_reference).is_none()
        && shared_borrowed_parts(program, local.type_reference)
            .is_some_and(|(referee, _)| program.primitive_type_reference(referee).is_none());
    if !mutable_view && !shared_view {
        parts(program, local.type_reference)?;
    }
    if local.is_mutable {
        return None;
    }
    let ExpressionNode::Call(expression) = program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    if expression.target_symbol != call.target_symbol
        || call.authored_expression != local.initial_value
        || call.statement_index != result.statement_index as usize
        || call.call_ordinal != 0
    {
        return None;
    }
    let callee = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|candidate| candidate.symbol == call.target_symbol)?;
    // The returned carrier either forwards a whole `&mut` ingress argument or
    // selects one declared leaf out of an owned record argument. Both lanes
    // must end in the same single result loan below.
    // A `&mut` view return borrows a projection out of its callee's `&mut`
    // storage, resolved by the tail-expression source walk; the `parts` and
    // shared-borrow lanes keep the whole-ingress and leaf rosters.
    let leaf = if mutable_view || shared_view {
        None
    } else {
        source_parameter(program, callee)
            .is_none()
            .then(|| source_leaf(program, callee))
            .flatten()
    };
    let position = if mutable_view {
        let analysis = mutable_view_source_parameter(program, facts, callee);
        analysis?
    } else if shared_view {
        let analysis = shared_view_source_parameter(program, facts, callee);
        analysis?
    } else {
        leaf.as_ref()
            .map(|(position, _)| *position)
            .or_else(|| source_parameter(program, callee))?
    };
    let argument = if mutable_view || shared_view {
        // The callee's `self` ingress arrives through the call's receiver,
        // not its argument span; authored non-self arguments follow it in
        // order.
        let callee_parameters = program.state_parameters(callee);
        if callee_parameters
            .get(position)
            .is_some_and(|parameter| parameter.is_self)
        {
            expression.receiver
        } else {
            let offset = usize::from(callee_parameters.iter().any(|parameter| parameter.is_self));
            let Some(&argument) = program
                .expression_table
                .expression_handles(expression.arguments)
                .get(position.saturating_sub(offset))
            else {
                return None;
            };
            argument
        }
    } else {
        let Some(&argument) = program
            .expression_table
            .expression_handles(expression.arguments)
            .get(position)
        else {
            return None;
        };
        argument
    };
    // A declared-field projection may select the argument carrier out of an
    // owned record local or out of a nested call's anonymous result:
    // `input.inner` and `forward_outer(outer).inner` both move the whole
    // `inner` subtree. Index, case-payload and computed roots stay opaque to
    // leaf custody.
    let mut members = Vec::new();
    let mut root = argument;
    if mutable_view || shared_view {
        // An explicit borrow passes the same captured place as an implicit
        // borrowed argument. Peel only the requested access, then rejoin the
        // authored root and the exact checked loan below; the wrapper itself
        // neither creates result custody nor changes its release boundary.
        if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(root) {
            let parameter = program.state_parameters(callee).get(position)?;
            let expected_access = if view_source_parameter_is_mutable(program, parameter) {
                language_semantics::ReferenceAccess::Mutable
            } else if view_source_parameter_is_readable(program, parameter) {
                language_semantics::ReferenceAccess::Shared
            } else {
                return None;
            };
            if borrow.access != expected_access {
                return None;
            }
            root = borrow.target;
        }
    }
    while let ExpressionNode::Member(member) = program.expression_table.expression(root) {
        members.push(member);
        root = member.receiver;
    }
    let expected_places = if let Some((_, leaf_source)) = leaf {
        // The argument is an owned record local, possibly through one
        // declared-field projection; the returned leaf's loan is the exact
        // captured leaf loan under that projection, and this call is that
        // loan's last use.
        let [
            path @ ..,
            checked_trees::CheckedUnitStructuralPathSegment::Referent,
        ] = leaf_source.path.as_slice()
        else {
            return None;
        };
        match program.expression_table.expression(root) {
            ExpressionNode::Name(actual) => {
                if actual.head_symbol != actual.symbol
                    || program
                        .expression_table
                        .name_path_members(actual.members)
                        .len()
                        != 1
                {
                    return None;
                }
                let (index, input) = program.state_local_declaration(state, actual.symbol)?;
                if input.is_mutable
                    || index >= call.statement_index
                    || !is_reference_record(program, input.type_reference)
                {
                    return None;
                }
                let (mut selected_path, projected) = declared_field_path(
                    program,
                    input.type_reference,
                    members.iter().rev().copied(),
                )?;
                if program
                    .state_parameters(callee)
                    .get(position)
                    .is_none_or(|parameter| {
                        program.normalized_type_identity(parameter.type_reference)
                            != program.normalized_type_identity(projected)
                    })
                {
                    return None;
                }
                selected_path.extend(path.iter().cloned());
                let loans =
                    local_record_loans(program, facts, machine, state, u32::try_from(index).ok()?)?;
                let mut matching = loans
                    .iter()
                    .filter(|(source, _)| source.path == selected_path);
                let (_, leaf_loan) = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                let leaf_loan = facts.borrow.loans.get(*leaf_loan);
                if leaf_loan.last_use_statement_index != call.statement_index {
                    return None;
                }
                vec![(
                    leaf_loan.root_symbol,
                    facts.borrow.loan_segments(leaf_loan).to_vec(),
                )]
            }
            ExpressionNode::Call(nested) => {
                // The carrier is the nested call's anonymous result: its
                // returned-leaf roster names which caller-side captured loan
                // the projected leaf replays through the nested actual.
                let nested_destination = program
                    .machines()
                    .iter()
                    .flat_map(|machine| program.machine_states(machine))
                    .find(|candidate| candidate.symbol == nested.target_symbol)?;
                if !is_reference_record(program, nested_destination.return_type) {
                    return None;
                }
                let (mut selected_path, projected) = declared_field_path(
                    program,
                    nested_destination.return_type,
                    members.iter().rev().copied(),
                )?;
                if program
                    .state_parameters(callee)
                    .get(position)
                    .is_none_or(|parameter| {
                        program.normalized_type_identity(parameter.type_reference)
                            != program.normalized_type_identity(projected)
                    })
                {
                    return None;
                }
                selected_path.extend(path.iter().cloned());
                let sources = returned_record_sources(program, nested_destination)?;
                let mut matching = sources.iter().filter(|source| source.path == selected_path);
                let nested_leaf = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                let leaf_loan = call_actual_leaf_loan(
                    program,
                    facts,
                    machine,
                    state,
                    u32::try_from(call.statement_index).ok()?,
                    nested,
                    nested_destination,
                    nested_leaf,
                )?;
                let leaf_loan = facts.borrow.loans.get(leaf_loan);
                if leaf_loan.last_use_statement_index != call.statement_index {
                    return None;
                }
                vec![(
                    leaf_loan.root_symbol,
                    facts.borrow.loan_segments(leaf_loan).to_vec(),
                )]
            }
            _ => return None,
        }
    } else if mutable_view || shared_view {
        // A returned loan retains the actual argument's exact backing, whether
        // that backing is a parameter, an invocation-local owner, or a view of
        // either. Reconstruct the entire captured path instead of admitting only
        // parameter roots or collapsing a field to its declaration symbol.
        // Borrow checking owns availability/access; activation and release below
        // must still match this particular call and result binding.
        let (root, segments) =
            mutable_source_place(program, machine, state, call.statement_index, argument)?;
        rebase_place_through_local_loans(
            program,
            facts,
            machine,
            state.symbol,
            call.statement_index,
            root,
            segments,
        )
    } else {
        let ExpressionNode::Name(actual) = program.expression_table.expression(root) else {
            return None;
        };
        if actual.head_symbol != actual.symbol
            || program
                .expression_table
                .name_path_members(actual.members)
                .len()
                != 1
            || !members.is_empty()
            || !program.state_parameters(state).iter().any(|parameter| {
                parameter.symbol == actual.symbol
                    && !parameter.is_const
                    && !parameter.is_self
                    && parts(program, parameter.type_reference).is_some()
                    && program.normalized_type_identity(parameter.type_reference)
                        == program.normalized_type_identity(local.type_reference)
            })
        {
            return None;
        }
        vec![(actual.symbol, Vec::new())]
    };
    let mut states = facts
        .borrow
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .filter(|candidate| {
            candidate.machine_symbol == machine && candidate.state_symbol == state.symbol
        });
    let Some(borrowing) = states.next() else {
        return None;
    };
    if states.next().is_some() {
        return None;
    }
    let mut loans = facts.borrow.loans.iter().filter(|(handle, loan)| {
        facts.borrow.state_owns_loan(borrowing, *handle) && loan.owner_symbol == local.symbol
    });
    let Some((handle, loan)) = loans.next() else {
        return None;
    };
    let expected_kind = if shared_view {
        checked_trees::BorrowAccessKind::Read
    } else {
        checked_trees::BorrowAccessKind::Mutable
    };
    if loans.next().is_some()
        || loan.statement_index != call.statement_index
        || !expected_places.iter().any(|(root, segments)| {
            *root == loan.root_symbol && segments.as_slice() == facts.borrow.loan_segments(loan)
        })
        || loan.kind != expected_kind
        || !facts.borrow.loan_owner_path(loan).is_empty()
    {
        return None;
    }
    let Some(flow) = state_flow(facts, machine, state.symbol) else {
        return None;
    };
    let mut activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(flow.borrow_activations)
        .iter()
        .filter(|activation| activation.loan == handle);
    let Some(activation) = activations.next() else {
        return None;
    };
    if activations.next().is_some()
        || activation.source
            != (checked_trees::FlowInvalidationSource::Statement {
                statement_index: loan.statement_index,
            })
    {
        return None;
    }
    release_boundary(program, facts, machine, state.symbol, handle)?;
    Some(handle)
}

/// Reconstruct the `&mut T` result's caller-side storage loan the same way
/// `result_loan` does for a primitive referee: the signature's declared
/// source applied at this exact call site — the receiver for a `&mut self`
/// callee (elision rule 3), else the one unambiguous reference argument —
/// replayed as the recorded loan's (root, segments). The callee's own
/// completion independently verifies the returned place's ingress; this
/// replay pins the caller's substitution, activation and weakening.
///
/// Name, member and whole-borrow actuals spell exact places; a local actual
/// resolves through the loans already recorded for it (the same rebase the
/// borrow tracker performs). Indexed actuals keep their index segment;
/// computed receivers, nested-call actuals and borrow-carrying owned
/// carriers stay opaque and reject the whole lane.
pub fn mutable_result_loan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<arena::Handle<checked_trees::BorrowLoanFact>> {
    let Some(StatementNode::LocalData(local)) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return None;
    };
    if local.is_mutable || mutable_borrowed_parts(program, local.type_reference).is_none() {
        return None;
    }
    let ExpressionNode::Call(expression) = program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    if expression.target_symbol != call.target_symbol
        || call.authored_expression != local.initial_value
        || call.statement_index != result.statement_index as usize
        || call.call_ordinal != 0
    {
        return None;
    }
    let Some(callee_machine) = program.machines().iter().find(|candidate| {
        program
            .machine_states(candidate)
            .iter()
            .any(|candidate_state| candidate_state.symbol == call.target_symbol)
    }) else {
        return None;
    };
    let Some(callee) = program
        .machine_states(callee_machine)
        .iter()
        .find(|candidate| candidate.symbol == call.target_symbol)
    else {
        return None;
    };
    if mutable_borrowed_parts(program, callee.return_type).is_none()
        || program.normalized_type_identity(local.type_reference)
            != program.normalized_type_identity(callee.return_type)
    {
        return None;
    }
    let Some(expected) = mutable_result_source_places(
        program,
        facts,
        machine,
        state,
        call.statement_index,
        expression,
        callee_machine,
        callee,
    ) else {
        return None;
    };
    let mut states = facts
        .borrow
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .filter(|candidate| {
            candidate.machine_symbol == machine && candidate.state_symbol == state.symbol
        });
    let borrowing = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let mut loans = facts.borrow.loans.iter().filter(|(handle, loan)| {
        facts.borrow.state_owns_loan(borrowing, *handle) && loan.owner_symbol == local.symbol
    });
    let Some((handle, loan)) = loans.next() else {
        return None;
    };
    if loans.next().is_some()
        || loan.statement_index != call.statement_index
        || loan.kind != checked_trees::BorrowAccessKind::Mutable
        || !facts.borrow.loan_owner_path(loan).is_empty()
        || !expected.iter().any(|(root, segments)| {
            *root == loan.root_symbol && segments.as_slice() == facts.borrow.loan_segments(loan)
        })
    {
        return None;
    }
    let flow = state_flow(facts, machine, state.symbol)?;
    let mut activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(flow.borrow_activations)
        .iter()
        .filter(|activation| activation.loan == handle);
    let activation = activations.next()?;
    if activations.next().is_some()
        || activation.source
            != (checked_trees::FlowInvalidationSource::Statement {
                statement_index: loan.statement_index,
            })
    {
        return None;
    }
    release_boundary(program, facts, machine, state.symbol, handle)?;
    Some(handle)
}

/// The caller-side places a `-> &mut T` call's declared return source can
/// name, spelled as recorded-loan (root, segments) candidates: the receiver
/// for a `&mut self` callee, else the one unambiguous reference argument's
/// actual, rebased through any loans the actual's root already carries. This
/// mirrors the resolution `helper_call_borrow_loan_place` performs so the
/// produced custody and this replay answer the same place.
pub fn mutable_result_source_places(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    call: &typed_trees::expression::TableCallExpression,
    callee_machine: &typed_trees::machine::Machine,
    callee: &typed_trees::state::State,
) -> Option<Vec<(SymbolHandle, Vec<facts::PlaceSegment>)>> {
    let parameters = program.state_parameters(callee);
    let mut candidates = Vec::new();
    // The callee's own completion names the authored parameter its `&mut`
    // tail rejoined: replay it here and substitute the caller's actual for
    // that position — the receiver for `self`, else the positional
    // non-receiver argument. This is the primary derivation: it is exact
    // for non-`self` return sources, which the elision rule below cannot
    // see once a `&mut self` receiver is present.
    if let Some((tail_index, tail_expression)) = mutable_return_tail(program, callee)
        && let Some((position, _)) = mutable_return_source(
            program,
            facts,
            callee_machine.symbol,
            callee,
            tail_index,
            tail_expression,
        )
        && let Some(actual) = if parameters[position].is_self {
            call.receiver.is_valid().then_some(call.receiver)
        } else {
            let index = parameters
                .iter()
                .take(position)
                .filter(|parameter| !parameter.is_self)
                .count();
            program
                .expression_table
                .expression_handles(call.arguments)
                .get(index)
                .copied()
        }
        && let Some((root, segments)) =
            mutable_source_place(program, machine, state, statement_index, actual)
    {
        candidates.extend(rebase_place_through_local_loans(
            program,
            facts,
            machine,
            state.symbol,
            statement_index,
            root,
            segments,
        ));
    }
    let actual = if parameters.iter().any(|parameter| parameter.is_self) {
        if !call.receiver.is_valid() {
            return if candidates.is_empty() {
                None
            } else {
                Some(candidates)
            };
        }
        Some(call.receiver)
    } else {
        let mut ref_positions: Vec<(usize, Option<String>)> = Vec::new();
        let mut position = 0usize;
        for parameter in parameters {
            if parameter.is_self {
                continue;
            }
            let index = position;
            position = position.saturating_add(1);
            if is_reference_node(program, parameter.type_reference) {
                ref_positions.push((
                    index,
                    reference_lifetime(program, parameter.type_reference).map(str::to_owned),
                ));
            } else if parameter_returns_borrow(program, parameter.type_reference) {
                // A borrow-carrying owned carrier resolves leaf by leaf; that
                // structural frontier names no single direct place.
                return if candidates.is_empty() {
                    None
                } else {
                    Some(candidates)
                };
            }
        }
        let position = match reference_lifetime(program, callee.return_type) {
            Some(output) => {
                let mut matching = ref_positions
                    .iter()
                    .filter(|(_, lifetime)| lifetime.as_deref() == Some(output));
                let Some((index, _)) = matching.next().filter(|_| matching.next().is_none()) else {
                    return if candidates.is_empty() {
                        None
                    } else {
                        Some(candidates)
                    };
                };
                Some(*index)
            }
            None => match ref_positions.as_slice() {
                [(single, _)] => Some(*single),
                _ => None,
            },
        };
        position.and_then(|position| {
            program
                .expression_table
                .expression_handles(call.arguments)
                .get(position)
                .copied()
        })
    };
    if let Some(actual) = actual
        && let Some((root, segments)) =
            mutable_source_place(program, machine, state, statement_index, actual)
    {
        candidates.extend(rebase_place_through_local_loans(
            program,
            facts,
            machine,
            state.symbol,
            statement_index,
            root,
            segments,
        ));
    }
    (!candidates.is_empty()).then_some(candidates)
}

/// The final `&mut` tail one `-> &mut T` state's authored body spells: a
/// terminal expression statement or an unconditional ordinary value
/// transition target, the same shapes `mutable_reference_result_operation`
/// produces `EstablishReference` for.
pub fn mutable_return_tail(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<(usize, typed_trees::expression::ExpressionHandle)> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let index = statements.len().checked_sub(1)?;
    match statements.get(index)? {
        StatementNode::Expression(expression) => Some((index, *expression)),
        StatementNode::Transition(transition)
            if transition.guard == TransitionGuardNode::Always
                && transition.exit == TransitionExit::Ordinary
                && !transition.continuation.is_valid() =>
        {
            let TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            Some((index, *expression))
        }
        _ => None,
    }
}

/// One actual's place spelled the way `borrow_access_place` spells it: borrows
/// unwrap to their target, members append the resolved field (pushing a `Case`
/// segment for payload fields), an attached-machine receiver roots at the
/// member itself, and index steps keep their normalized selector segment.
/// Every other shape stays opaque to the result loan.
fn mutable_source_place(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            mutable_source_place(program, machine, state, statement_index, inner.target)
        }
        ExpressionNode::Indexed(indexed) => {
            let (root, mut segments) =
                mutable_source_place(program, machine, state, statement_index, indexed.collection)?;
            segments.push(index_place_segment(program, indexed.index));
            Some((root, segments))
        }
        ExpressionNode::Member(member) => {
            let member_symbol = mutable_effective_member_symbol(
                program,
                machine,
                state,
                statement_index,
                member.receiver,
                member,
            );
            if !member_symbol.is_valid() {
                return None;
            }
            match program.expression_table.expression(member.receiver) {
                ExpressionNode::Name(path)
                    if path.members.count() == 1
                        && mutable_name_root(
                            program,
                            state,
                            statement_index,
                            member.receiver,
                            path,
                        ) == Some(machine) =>
                {
                    Some((member_symbol, Vec::new()))
                }
                _ => {
                    let (root, mut segments) = mutable_source_place(
                        program,
                        machine,
                        state,
                        statement_index,
                        member.receiver,
                    )?;
                    push_field_place_segments(program, &mut segments, member_symbol);
                    Some((root, segments))
                }
            }
        }
        ExpressionNode::Name(path) => {
            if crate::proof_contracts::bound_expression_meaning::exact_case_reference_owner(
                program, expression,
            )
            .is_some()
            {
                return None;
            }
            let root = mutable_name_root(program, state, statement_index, expression, path)?;
            let member_symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let skip = member_symbols
                .iter()
                .position(|member_symbol| *member_symbol == root)
                .map(|index| index + 1)
                .unwrap_or(1);
            let mut segments = Vec::new();
            for symbol in member_symbols.iter().skip(skip).copied() {
                push_field_place_segments(program, &mut segments, symbol);
            }
            Some((root, segments))
        }
        _ => None,
    }
}

/// The root symbol one `Name` resolves to: the typed path's first retained
/// symbol, else the self receiver's parameter, a same-named parameter, or the
/// latest local spelling the name before this statement.
fn mutable_name_root(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    path: &typed_trees::expression::TableNamePath,
) -> Option<SymbolHandle> {
    if let Some(symbol) = program
        .expression_table
        .name_path_member_symbols(path.member_symbols)
        .first()
        .copied()
        .filter(|symbol| symbol.is_valid())
        .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
        .or_else(|| path.symbol.is_valid().then_some(path.symbol))
    {
        return Some(symbol);
    }
    let name = program.expression_table.display_name(expression);
    if language_core::is_self_receiver(&name) {
        return program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .map(|parameter| parameter.symbol);
    }
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| !parameter.is_self && parameter.name.as_str() == name)
    {
        return Some(parameter.symbol);
    }
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == name => Some(local.symbol),
            _ => None,
        })
}

/// An index step's normalized segment: a literal or constant-valued index is
/// its fixed ordinal, a half-open literal range is a window, and anything
/// else stays a runtime `Index` expression.
fn index_place_segment(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> facts::PlaceSegment {
    if let ExpressionNode::Range(range) = program.expression_table.expression(expression)
        && let Some((start, end)) = fixed_half_open_range(program, range)
    {
        return facts::PlaceSegment::FixedRange { start, end };
    }
    program
        .expression_table
        .constant_integer_value(expression)
        .and_then(|value| usize::try_from(value).ok())
        .map(|index| facts::PlaceSegment::FixedIndex { index })
        .unwrap_or(facts::PlaceSegment::Index { expression })
}

fn fixed_half_open_range(
    program: &TypedTrees,
    range: &typed_trees::expression::TableRangeExpression,
) -> Option<(usize, usize)> {
    let start = if range.start.is_valid() {
        usize::try_from(
            program
                .expression_table
                .constant_integer_value(range.start)?,
        )
        .ok()?
    } else {
        0
    };
    if !range.end.is_valid() {
        return None;
    }
    let end = usize::try_from(program.expression_table.constant_integer_value(range.end)?).ok()?;
    let end = if range.end_inclusive {
        end.checked_add(1)?
    } else {
        end
    };
    Some((start, end))
}

/// `borrow_access_place`'s member spelling: the member resolved through the
/// receiver's own type — an attached machine's `self` data, a parameter's
/// named type, or the data a previous hop landed in — before falling back to
/// the expression's retained symbol. `self.bag` in an attached machine
/// spells the data field `Main::bag`, not the machine's `Main::main::bag`
/// state spelling, so both resolvers must answer the same symbol.
fn mutable_effective_member_symbol(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    receiver: typed_trees::expression::ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> SymbolHandle {
    if let Some(type_symbol) =
        mutable_receiver_type_symbol(program, machine, state, statement_index, receiver)
        && let Some(symbol) =
            mutable_member_from_type_symbol(program, type_symbol, member.member.as_str())
    {
        return symbol;
    }
    member.member_symbol
}

/// The named type one receiver expression's value inhabits: borrows unwrap,
/// indexed collections keep their element's named owner, and names and
/// members resolve through the symbols their roots already spell.
fn mutable_receiver_type_symbol(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            mutable_receiver_type_symbol(program, machine, state, statement_index, inner.target)
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = mutable_receiver_type_symbol(
                program,
                machine,
                state,
                statement_index,
                indexed.collection,
            )?;
            let definition = crate::program_scope::data_definitions::data_definition_by_symbol(
                program, collection,
            )?;
            Some(definition.symbol)
        }
        ExpressionNode::Member(member) => {
            let symbol = mutable_effective_member_symbol(
                program,
                machine,
                state,
                statement_index,
                member.receiver,
                member,
            );
            mutable_symbol_type_symbol(program, machine, state, statement_index, symbol)
        }
        ExpressionNode::Name(path) => {
            let root = mutable_name_root(program, state, statement_index, expression, path)?;
            mutable_symbol_type_symbol(program, machine, state, statement_index, root)
        }
        _ => None,
    }
}

/// The named type a resolved root or member symbol's value inhabits: the
/// attached machine's own data for its `self` root, the declared type of a
/// parameter or earlier local, or the named owner of a data field's type.
fn mutable_symbol_type_symbol(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if symbol == machine {
        let owner = program
            .machines()
            .iter()
            .find(|owner| owner.symbol == machine)?;
        return program
            .attached_data_definition(owner)
            .map(|definition| definition.symbol);
    }
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
    {
        let type_symbol = program
            .type_reference_table
            .type_symbol(parameter.type_reference);
        return type_symbol.is_valid().then_some(type_symbol);
    }
    if let Some(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
            _ => None,
        })
    {
        let type_symbol = program
            .type_reference_table
            .type_symbol(local.type_reference);
        return type_symbol.is_valid().then_some(type_symbol);
    }
    program
        .data_definitions()
        .iter()
        .flat_map(|definition| program.data_members(definition))
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                let type_symbol = program
                    .type_reference_table
                    .type_symbol(field.type_reference);
                type_symbol.is_valid().then_some(type_symbol)
            }
            typed_trees::data::DataMember::Variant(variant) => program
                .data_payload_fields(variant)
                .iter()
                .find(|field| field.symbol == symbol)
                .and_then(|field| {
                    let type_symbol = program
                        .type_reference_table
                        .type_symbol(field.type_reference);
                    type_symbol.is_valid().then_some(type_symbol)
                }),
            _ => None,
        })
}

/// A member name resolved inside one named type — `Main::bag` for `bag` in
/// `Main` — covering declared fields, variants and payload fields of a data
/// definition, an attached machine's data, and machine-owned data, exactly
/// as `resolve_member_symbol_from_type_symbol` spells them.
fn mutable_member_from_type_symbol(
    program: &TypedTrees,
    type_symbol: SymbolHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let resolved = |definition: &typed_trees::data::DataDefinition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    Some(field.symbol)
                }
                typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    Some(variant.symbol)
                }
                typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.name.as_str() == member_name)
                    .map(|field| field.symbol),
                _ => None,
            })
    };
    if let Some(definition) =
        crate::program_scope::data_definitions::data_definition_by_symbol(program, type_symbol)
        && let Some(symbol) = resolved(definition)
    {
        return Some(symbol);
    }
    if let Some(owner) = program
        .machines()
        .iter()
        .find(|owner| owner.symbol == type_symbol)
    {
        if let Some(definition) = program.attached_data_definition(owner)
            && let Some(symbol) = resolved(definition)
        {
            return Some(symbol);
        }
        if let Some(owned) = program
            .machine_owned_data(owner)
            .iter()
            .find(|owned| owned.name.as_str() == member_name)
        {
            return Some(owned.symbol);
        }
    }
    None
}

/// A field step pushes the payload's `Case` segment first when the field
/// lives under a sum case, then the field itself — the spelling
/// `push_field_place_segments` shares for place construction.
fn push_field_place_segments(
    program: &TypedTrees,
    segments: &mut Vec<facts::PlaceSegment>,
    symbol: SymbolHandle,
) {
    if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
        segments.push(facts::PlaceSegment::Case { variant });
    }
    segments.push(facts::PlaceSegment::Field { symbol });
}

/// Rebase one actual place through the loans the actual's root already
/// carries: a view local's own loan names the loaned storage this new place
/// joins. Each qualifying earlier loan yields one candidate; none means the
/// root names the place directly (a parameter's own storage).
fn rebase_place_through_local_loans(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    root: SymbolHandle,
    segments: Vec<facts::PlaceSegment>,
) -> Vec<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    let mut states = facts
        .borrow
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .filter(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state);
    let Some(borrowing) = states.next().filter(|_| states.next().is_none()) else {
        return Vec::new();
    };
    let rebased: Vec<(SymbolHandle, Vec<facts::PlaceSegment>)> = facts
        .borrow
        .loans
        .iter()
        .filter(|(handle, loan)| {
            facts.borrow.state_owns_loan(borrowing, *handle)
                && loan.owner_symbol == root
                && loan.statement_index < statement_index
                && loan_owner_path_is_prefix(program, facts.borrow.loan_owner_path(loan), &segments)
        })
        .map(|(_, loan)| {
            let mut rebased = facts.borrow.loan_segments(loan).to_vec();
            rebased.extend_from_slice(&segments[facts.borrow.loan_owner_path(loan).len()..]);
            (loan.root_symbol, rebased)
        })
        .collect();
    if rebased.is_empty() {
        vec![(root, segments)]
    } else {
        rebased
    }
}

/// The authored `&mut place` tail of a `-> &mut T` return, replayed the way
/// `mutable_reference_result_operation` spells it: the tail place resolves
/// through `mutable_source_place` and any loans the root already carries
/// (a view local's own `&mut [T]` names the loaned storage it views), then
/// must bottom out at exactly the declared referee below one `&mut`
/// parameter this state retains. An attached-machine member root spells
/// `self.<field>` with the field as root and no `self` segment, so that
/// candidate rejoins under the `self` parameter. The answer is the
/// parameter's authored position and the projected path.
pub fn mutable_return_source(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(usize, Vec<checked_trees::CheckedUnitStructuralPathSegment>)> {
    let (referee, access) = mutable_borrowed_parts(program, state.return_type)?;
    if access != CheckedStructuralAccess::MutableBorrow {
        return None;
    }
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
        return None;
    };
    if borrow.access != language_semantics::ReferenceAccess::Mutable {
        return None;
    }
    let (root, segments) =
        mutable_source_place(program, machine, state, statement_index, borrow.target)?;
    let candidates = rebase_place_through_local_loans(
        program,
        facts,
        machine,
        state.symbol,
        statement_index,
        root,
        segments,
    );
    let parameters = program.state_parameters(state);
    let self_position = parameters.iter().position(|parameter| parameter.is_self);
    let mut matches = candidates.iter().filter_map(|(root, segments)| {
        let mut sources = Vec::new();
        if let Some(position) = parameters
            .iter()
            .position(|parameter| parameter.symbol == *root)
        {
            sources.push((position, segments.clone()));
        }
        if let Some(position) = self_position {
            let mut rebased = Vec::with_capacity(segments.len() + 1);
            rebased.push(facts::PlaceSegment::Field { symbol: *root });
            rebased.extend_from_slice(segments);
            sources.push((position, rebased));
        }
        let mut retained = sources.iter().filter_map(|(position, segments)| {
            let carrier = &parameters[*position];
            // `is_mutable` records the `&mut` spelling itself, the same
            // mutability `mutable_reference_result_operation` reads:
            // reseating is only a hazard for a mutable binding on a
            // non-reference carrier. The reference's own readability
            // stays exact through `mutable_borrowed_parts` below.
            if carrier.is_mutable && !is_reference_node(program, carrier.type_reference) {
                return None;
            }
            let Some((carrier_referee, carrier_access)) =
                mutable_borrowed_parts(program, carrier.type_reference)
            else {
                return None;
            };
            if carrier_access != CheckedStructuralAccess::MutableBorrow {
                return None;
            }
            let (storage, path) =
                mutable_return_path(program, machine, carrier_referee, segments.as_slice())?;
            let identity_match = program.normalized_type_identity(storage)
                == program.normalized_type_identity(referee);
            identity_match.then_some((*position, path))
        });
        let found = retained.next()?;
        retained.next().is_none().then_some(found)
    });
    let Some(found) = matches.next() else {
        return None;
    };
    matches.next().is_none().then_some(found)
}

/// The typed projection a `&mut` return source spells: each field resolves
/// against its named owner and each fixed index against its literal fixed
/// array length, with constrained and reference shells peeled exactly as
/// `projected_argument_path` peels them before the same check. The answer is
/// the projected leaf type and the retained structural path.
fn mutable_return_path(
    program: &TypedTrees,
    machine: SymbolHandle,
    mut reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Option<(
    TypeReferenceHandle,
    Vec<checked_trees::CheckedUnitStructuralPathSegment>,
)> {
    let mut path = Vec::with_capacity(segments.len());
    for segment in segments {
        loop {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. }
                | TypeReferenceNode::Reference {
                    referee: base_type, ..
                } => reference = *base_type,
                _ => break,
            }
        }
        match (
            segment,
            program.type_reference_table.type_reference(reference),
        ) {
            (
                facts::PlaceSegment::Field { symbol },
                TypeReferenceNode::Named {
                    symbol: owner,
                    name,
                },
            ) => {
                // `self`'s referee names the machine's `Self` alias, whose
                // symbol is the machine itself — the same correspondence
                // `correspondence_data_type` spells — rather than the
                // attached data definition's symbol.
                let owner = if *owner == machine && name.as_str() == "Self" {
                    program
                        .machines()
                        .iter()
                        .find(|candidate| candidate.symbol == machine)?
                        .attached_data_symbol
                } else {
                    *owner
                };
                let definition = crate::program_scope::data_definitions::data_definition_by_symbol(
                    program, owner,
                )?;
                let field = program.data_members(definition).iter().find_map(|member| {
                    let typed_trees::data::DataMember::Field(field) = member else {
                        return None;
                    };
                    (field.symbol == *symbol).then_some(field)
                })?;
                if field.relevance != language_core::BindingRelevance::Relevant {
                    return None;
                }
                path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    field.path_identity(),
                ));
                reference = field.type_reference;
            }
            (
                facts::PlaceSegment::FixedIndex { index },
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                },
            ) if *index < *length => {
                path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).ok()?,
                ));
                reference = *element_type;
            }
            _ => return None,
        }
    }
    Some((reference, path))
}

/// Whether a recorded loan's owner path is a prefix of one actual's place
/// segments, so the loan's own (root, segments) rebase the actual's
/// remainder. Dynamic owners overlap any index-shaped place step.
fn loan_owner_path_is_prefix(
    program: &TypedTrees,
    owner_path: &[checked_trees::BorrowLoanOwnerSegment],
    segments: &[facts::PlaceSegment],
) -> bool {
    owner_path.len() <= segments.len()
        && owner_path
            .iter()
            .zip(segments.iter())
            .all(|(owner, place)| {
                use checked_trees::BorrowLoanOwnerSegment as Owner;
                match (owner, place) {
                    (Owner::Field(owner_symbol), facts::PlaceSegment::Field { symbol }) => {
                        !symbol.is_valid() || owner_symbol == symbol
                    }
                    (Owner::Case(owner_variant), facts::PlaceSegment::Case { variant }) => {
                        owner_variant == variant
                    }
                    (Owner::FixedIndex(owner_index), facts::PlaceSegment::FixedIndex { index }) => {
                        owner_index == index
                    }
                    (Owner::FixedIndex(owner_index), facts::PlaceSegment::Index { expression }) => {
                        program
                            .expression_table
                            .constant_integer_value(*expression)
                            .and_then(|value| usize::try_from(value).ok())
                            .is_none_or(|place_index| *owner_index == place_index)
                    }
                    (
                        Owner::DynamicIndex,
                        facts::PlaceSegment::FixedIndex { .. }
                        | facts::PlaceSegment::FixedRange { .. }
                        | facts::PlaceSegment::Index { .. },
                    ) => true,
                    _ => false,
                }
            })
}

/// Whether a type reference is a reference, seeing through `Constrained`
/// shells exactly like the elision classifier does.
fn is_reference_node(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => is_reference_node(program, *base_type),
        _ => false,
    }
}

/// The explicit lifetime name on a reference type, seeing through a
/// `Constrained` shell. `None` for an elided reference or a non-reference.
fn reference_lifetime<'a>(
    program: &'a TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<&'a str> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { lifetime, .. } => {
            lifetime.as_ref().map(|name| name.as_str())
        }
        TypeReferenceNode::Constrained { base_type, .. } => reference_lifetime(program, *base_type),
        _ => None,
    }
}

/// Whether one non-reference parameter type structurally carries a borrow:
/// a record, array, or slice containing a reference field somewhere. That
/// shape resolves through the structural leaf frontier, not this lane's
/// direct source.
fn parameter_returns_borrow(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    fn visit(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        seen: &mut Vec<SymbolHandle>,
    ) -> Option<bool> {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } => Some(true),
            TypeReferenceNode::Constrained { base_type, .. } => visit(program, *base_type, seen),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => visit(program, *element_type, seen),
            TypeReferenceNode::Named { symbol, .. } => {
                if seen.contains(symbol) {
                    return None;
                }
                let data = crate::program_scope::data_definitions::data_definition_by_symbol(
                    program, *symbol,
                )?;
                seen.push(*symbol);
                let mut carries = false;
                for member in program.data_members(data) {
                    let typed_trees::data::DataMember::Field(field) = member else {
                        return None;
                    };
                    carries |= visit(program, field.type_reference, seen)?;
                }
                seen.pop();
                Some(carries)
            }
            _ => Some(false),
        }
    }
    visit(program, reference, &mut Vec::new()) == Some(true)
}

pub fn release_statement(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<u32> {
    let flow = state_flow(facts, machine, state)?;
    let mut weakenings = facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings)
        .iter()
        .filter(|weakening| weakening.loan == handle);
    let weakening = weakenings.next()?;
    let checked_trees::FlowInvalidationSource::Statement { statement_index } = weakening.source
    else {
        return None;
    };
    if weakenings.next().is_some()
        || !matches!(
            weakening.reason,
            checked_trees::FlowBorrowWeakeningReason::LastUseExpired
                | checked_trees::FlowBorrowWeakeningReason::StateExit
        )
        || statement_index
            != facts
                .borrow
                .loans
                .get(handle)
                .last_use_statement_index
                .checked_add(1)?
    {
        return None;
    }
    u32::try_from(statement_index).ok()
}

/// The weakening statement a borrowed result's loan release lands on, or the
/// frame's own edge when the loan's last use is the final statement and no
/// weakening had to restore access — the state exit retires that custody
/// implicitly. Every release-side query shares this answer so the produced
/// operations, their custody accounting and the lowered validation see one
/// boundary.
pub fn release_boundary(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<u32> {
    if let Some(boundary) = release_statement(facts, machine, state, handle) {
        return Some(boundary);
    }
    let state = program
        .machines()
        .iter()
        .flat_map(|candidate| program.machine_states(candidate))
        .find(|candidate| candidate.symbol == state)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    (facts
        .borrow
        .loans
        .get(handle)
        .last_use_statement_index
        .checked_add(1)?
        >= statements.len())
    .then(|| u32::try_from(statements.len()).ok())
    .flatten()
}

fn state_flow(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<&checked_trees::FlowStateFact> {
    let mut states = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .filter(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state);
    let flow = states.next()?;
    states.next().is_none().then_some(flow)
}
