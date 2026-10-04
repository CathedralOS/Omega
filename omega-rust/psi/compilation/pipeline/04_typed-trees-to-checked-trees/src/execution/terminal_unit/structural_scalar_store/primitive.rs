//! Primitive leaves: whole primitive storage and primitive elements.
//!
//! A primitive leaf is written by `WriteOnlyPrimitiveStore` over one checked
//! path from the shared destination resolver's borrowed or owned storage.
//! The path composes fields, explicit reference crossings, literal elements, and the
//! target's runtime elements: `self.cells[self.i] = v` is the store over
//! `[cells, RuntimeIndex(AssignmentIndex { depth: 0 })]` and
//! `self.grid[i][j] = v` the store over `[grid, RuntimeIndex(depth 1),
//! RuntimeIndex(depth 0)]`, not a separate indexed store with a selector as
//! an operand. The planner proves no bound for a runtime element: each
//! selector is the statement's retained `AssignmentIndex { depth }` scalar,
//! and Terminal re-proves `index < extent` for the segment's obligation from
//! the facts that hold at the store (entry requires, dominating guards,
//! stored-field snapshots), so a selector the planner cannot bound
//! syntactically still composes and an unproven one still fails
//! verification rather than being trusted here.
//!
//! A closed array literal stored into a primitive-array field is the same
//! store once per element, over the field's path plus that element's
//! literal indices (`array_literal_stores`). An indexed row destination
//! (`self.grid[i] = [1, 2, 3]`) is that store over the same leaf roster with
//! the runtime element already in the path: the selector's bound is re-proven
//! exactly as for an element store.
use super::super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment, DataMember, Multiplicity,
    PrimitiveType, StatementNode, SymbolHandle, TypeConstraintNode, TypeReferenceNode, TypedTrees,
};

/// Find primitive storage established by an initializer or its declaration's
/// plain zero value before this occurrence.
pub(in crate::execution::terminal_unit) fn primitive_local_before<'program>(
    program: &'program TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<&'program typed_trees::statement::TableLocalData> {
    let (_, local) = program
        .state_local_declaration(state, symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index)?;
    if !local.is_mutable
        || (!program
            .expression_table
            .expression_is_valid(local.initial_value)
            && crate::values::zeroed_scalar_local_initializer(program, local).is_none())
        || !(matches!(
            program
                .type_reference_table
                .type_reference(local.type_reference),
            TypeReferenceNode::Named { .. }
        ) || validation::is_arithmetic_policy_only_integer(program, local.type_reference))
        || program
            .primitive_type_reference(local.type_reference)
            .is_none()
    {
        return None;
    }
    Some(local)
}

/// The primitive store of one authored assignment, or `None` when its target
/// is not a primitive leaf this lane writes. The shared statement roster
/// validates the complete state write frame.
pub(super) fn store_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &super::destination::Destination<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<CheckedUnitEffectOperationPlan> {
    let projection = destination.storage_projection(program)?;
    let primitive_type = program.primitive_type_reference(projection.type_reference)?;
    // Declared scalar fields retain their own field-store proof obligations.
    // Following a reference instead reaches primitive storage, regardless of
    // whether the pointer lives at the root or inside another owned value.
    if projection.ends_at_field || destination.recast_view.is_some() {
        return None;
    }
    primitive_leaf(program, machine, state, assignment.target)?;
    if projection.path.is_empty() {
        match destination.root {
            super::destination::Root::Parameter { plan, .. }
                if plan.is_self || plan.multiplicity != Multiplicity::Unrestricted =>
            {
                return None;
            }
            super::destination::Root::Local { symbol } => {
                primitive_local_before(program, state, statement_index as usize, symbol)?;
            }
            _ => {}
        }
    }
    let authored = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index as usize,
        assignment.target,
    )?;
    let facts::PlaceRoot::Symbol(authored_symbol) = authored.root else {
        return None;
    };
    let value = primitive_store_value(
        program,
        facts,
        machine,
        state,
        statement_index,
        assignment,
        authored_symbol,
        authored.segments.is_empty(),
        primitive_type,
    )?;
    Some(CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
        statement_index,
        destination: destination.root.destination(),
        path: projection.path,
        value,
    })
}

/// The stored value of one primitive store: the statement's own call result
/// or the pure scalar bound at `AssignmentValue`, retagged to the leaf's
/// declared primitive. Whole-root stores keep the authored root symbol as
/// the bound destination; every element or field store binds `invalid`.
fn primitive_store_value(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    authored_symbol: SymbolHandle,
    authored_whole: bool,
    primitive_type: PrimitiveType,
) -> Option<checked_trees::CheckedCallScalarArgument> {
    // Computed RHS values share the ordinary scalar evaluator. Keep the
    // authored assignment root until emission completes it, then replace storage.
    let computations = &facts.values.scalar_computations;
    let mut roots = computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.state == state.symbol
                && root.statement_ordinal == statement_index
                && root.role == CheckedScalarExpressionRole::AssignmentValue
        });
    if let Some(root) = roots.next() {
        if roots.next().is_some()
            || root.machine != machine.symbol
            || !computations.nodes.is_valid(root.root)
            || computations.nodes.get(root.root).authored_root != assignment.value
            || computations.nodes.get(root.root).primitive_type != primitive_type
            || facts
                .values
                .scalar_expressions
                .expression_at(
                    state.symbol,
                    statement_index,
                    CheckedScalarExpressionRole::AssignmentValue,
                )
                .is_some()
        {
            return None;
        }
        return Some(checked_trees::CheckedCallScalarArgument::Computation(
            root.root,
        ));
    }
    let Some((binding, value)) = facts.values.scalar_expressions.bound_expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
    ) else {
        return None;
    };
    if binding.expression != assignment.value
        || binding.destination
            != if authored_whole {
                authored_symbol
            } else {
                SymbolHandle::invalid()
            }
        || crate::values::scalar_expression_type(value) != Some(primitive_type)
        || !scalar_store_value_is_exact(program, facts, state, binding, value, primitive_type)
    {
        return None;
    }
    Some(checked_trees::CheckedCallScalarArgument::Pure(
        value.clone(),
    ))
}

/// The stores of a closed array literal written whole into a primitive-array
/// field of exclusive borrowed or owned local storage: one `WriteOnlyPrimitiveStore`
/// per row-major element, over the field's path plus that element's literal
/// indices. Every element must be a closed value -- literals, parameters,
/// locals and their compositions -- so no element observes a store the
/// literal has already made; one that reads storage keeps declining.
pub(super) fn array_literal_stores(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &super::destination::Destination<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let expected = validation::closed_array_store_type(
        program,
        validation::declared_place_type_raw(program, machine, Some(state), assignment.target)?,
    )?;
    let extents = closed_array_extents(program, expected)?;
    let projection = destination.storage_projection(program)?;
    if projection.path.is_empty()
        || !validation::place_has_builtin_coordinates(
            program,
            machine,
            Some(state),
            assignment.target,
        )
    {
        return None;
    }
    let path = projection.path;
    let destination = destination.root.destination();
    let elements = super::super::control::scalar_arrays::elements(
        program,
        facts,
        machine.symbol,
        state.symbol,
        statement_index,
        checked_trees::CheckedArrayConstructionSource::Statement,
        assignment.value,
        expected,
    )?;
    elements
        .into_iter()
        .enumerate()
        .map(|(ordinal, value)| {
            let checked_trees::CheckedCallScalarArgument::Pure(expression) = &value else {
                return None;
            };
            if !scalar_expression_needs_no_bindings(expression) {
                return None;
            }
            let mut element_path = path.clone();
            element_path.extend(
                row_major_indices(u64::try_from(ordinal).ok()?, &extents)?
                    .into_iter()
                    .map(CheckedUnitStructuralPathSegment::FixedIndex),
            );
            Some(CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                statement_index,
                destination,
                path: element_path,
                value,
            })
        })
        .collect()
}

/// The literal extents of a closed primitive array type, outermost first.
fn closed_array_extents(
    program: &TypedTrees,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> Option<Vec<u64>> {
    if !validation::is_closed_primitive_array_type(program, reference) {
        return None;
    }
    let mut extents = Vec::new();
    loop {
        match program
            .type_reference_table
            .type_reference(validation::peel_arithmetic_policy(program, reference))
        {
            TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } => {
                extents.push(u64::try_from(*length).ok()?);
                reference = *element_type;
            }
            _ => break,
        }
    }
    (!extents.is_empty()).then_some(extents)
}

/// The per-dimension indices of the `ordinal`th row-major element.
fn row_major_indices(mut ordinal: u64, extents: &[u64]) -> Option<Vec<u64>> {
    let mut indices = vec![0; extents.len()];
    for (index, extent) in indices.iter_mut().zip(extents).rev() {
        *index = ordinal.checked_rem(*extent)?;
        ordinal /= extent;
    }
    (ordinal == 0).then_some(indices)
}

/// The assignment leaf's declared type must be a named primitive, after the
/// arithmetic-policy shell peel.
fn primitive_leaf(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<PrimitiveType> {
    let leaf = primitive_leaf_type_reference(program, machine, state, expression)?;
    validation::unrestricted_builtin_primitive(program, leaf)
}

/// The type of the primitive leaf an assignment through `expression` writes.
/// An arithmetic-policy shell (`i32 in Wrapping`) qualifies the element's
/// operations, not its storage identity, so it peels here like the primitive
/// leaf of the record-field store route. Range, named, and domain
/// constraints carry their own write obligations and keep their own owners.
/// An exclusive borrow carrier peels to its referee the same way: `borrowed`
/// in `let borrowed: &mut u8 = &mut self.buf[i]` spells the reference type,
/// while the store reaches the element behind it. Whether that reach is
/// lawful is the destination resolver's call, not the leaf walker's — a
/// carrier without validated formation keeps its own root and declines
/// there. A shared borrow never reaches this lane.
fn primitive_leaf_type_reference(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let mut leaf = validation::declared_place_type_raw(program, machine, Some(state), expression)?;
    loop {
        match program.type_reference_table.type_reference(leaf) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                if !program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .all(|constraint| matches!(constraint, TypeConstraintNode::ArithmeticDomain(_)))
                {
                    break;
                }
                leaf = *base_type;
            }
            TypeReferenceNode::Reference {
                access, referee, ..
            } => {
                if matches!(access, language_semantics::ReferenceAccess::Shared) {
                    break;
                }
                leaf = *referee;
            }
            _ => break,
        }
    }
    Some(leaf)
}

/// Literal path segments keep their exact source identity; fields reject an
/// erased or domain-constrained declaration; a runtime element names its
/// selector's retained coordinate.
pub(in crate::execution::terminal_unit) fn checked_unit_path(
    program: &TypedTrees,
    selectors: &super::selectors::TargetSelectors,
    segments: &[facts::PlaceSegment],
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    segments
        .iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Index { expression } => selectors.runtime_segment(*expression),
            facts::PlaceSegment::FixedIndex { index } => Some(
                CheckedUnitStructuralPathSegment::FixedIndex(u64::try_from(*index).ok()?),
            ),
            facts::PlaceSegment::Field { symbol } => {
                let mut fields = program
                    .data_definitions()
                    .iter()
                    .flat_map(|owner| program.data_members(owner))
                    .filter_map(|member| match member {
                        DataMember::Field(field) if field.symbol == *symbol => Some(field),
                        _ => None,
                    });
                let field = fields.next()?;
                if fields.next().is_some()
                    || field.relevance.is_erased()
                    || !crate::facts::field_domain::domain_constraint_symbols(
                        program,
                        field.type_reference,
                    )
                    .is_empty()
                {
                    return None;
                }
                Some(CheckedUnitStructuralPathSegment::Field(
                    field.path_identity(),
                ))
            }
            _ => None,
        })
        .collect()
}

/// The scalar-binding closure both store custody proofs share: the binding
/// must close over exactly the primitive parameters and the immutable
/// primitive locals declared before its statement — nothing missing,
/// nothing extra.
fn scalar_binding_symbols_exact(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    binding: &checked_trees::CheckedScalarExpressionBindings,
) -> bool {
    let Ok(statement_index) = usize::try_from(binding.statement_ordinal) else {
        return false;
    };
    let Some(prefix) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(..statement_index)
    else {
        return false;
    };
    let symbols = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .map(|parameter| parameter.symbol)
        .chain(prefix.iter().filter_map(|statement| {
            match statement {
                StatementNode::LocalData(local)
                    if !local.is_mutable
                        && program
                            .primitive_type_reference(local.type_reference)
                            .is_some() =>
                {
                    Some(local.symbol)
                }
                _ => None,
            }
        }));
    symbols.eq(facts
        .values
        .scalar_expressions
        .binding_symbols
        .span_or_empty(binding.symbols)
        .iter()
        .copied())
}

pub(in crate::execution::terminal_unit) fn scalar_custody_is_exact(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    binding: &checked_trees::CheckedScalarExpressionBindings,
    value: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> bool {
    if !scalar_binding_symbols_exact(program, facts, state, binding) {
        return false;
    }
    let Ok(statement_index) = usize::try_from(binding.statement_ordinal) else {
        return false;
    };
    if binding.role == CheckedScalarExpressionRole::StorageInitializer
        && !binding.expression.is_valid()
    {
        return matches!(program.statement_table.statements(state.statement_nodes).get(statement_index),
            Some(StatementNode::LocalData(local)) if local.symbol == binding.destination
                && crate::values::zeroed_scalar_local_initializer(program, local).as_ref() == Some(value));
    }
    crate::values::lower_unit_scalar_argument(
        program,
        &facts.operators,
        state,
        statement_index,
        binding.expression,
        primitive_type,
    )
    .as_ref()
        == Some(value)
}

/// A store's scalar value is exact when its binding closes over precisely
/// the scalar frame (as `scalar_custody_is_exact` demands) and the retained
/// expression either re-lowers through the unit-argument vocabulary — as
/// local custody requires — or simply cannot re-lower there at all. The
/// scalar-expression table is itself the canonical lowering of this
/// authored root; named-domain casts and other shapes the narrower
/// argument lowerer cannot spell keep their retained value, as long as
/// that value still lowers through the ordinary scalar vocabulary.
fn scalar_store_value_is_exact(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    binding: &checked_trees::CheckedScalarExpressionBindings,
    value: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> bool {
    if !scalar_binding_symbols_exact(program, facts, state, binding) {
        return false;
    }
    let Ok(statement_index) = usize::try_from(binding.statement_ordinal) else {
        return false;
    };
    match crate::values::lower_unit_scalar_argument(
        program,
        &facts.operators,
        state,
        statement_index,
        binding.expression,
        primitive_type,
    ) {
        Some(relowered) => relowered == *value,
        None => scalar_expression_needs_no_bindings(value),
    }
}

/// A retained scalar value stands on its own when every node lowers through
/// the ordinary scalar vocabulary — parameters, locals, literals and their
/// compositions. Structural field/indexed reads, storage reads, erased
/// formals and trapping casts resolve through binding lists or runtime
/// policy that the store's value lane does not carry, so a retained
/// expression built from any of them must keep declining.
fn scalar_expression_needs_no_bindings(
    expression: &checked_trees::CheckedScalarExpression,
) -> bool {
    use checked_trees::CheckedScalarExpression;
    match expression {
        CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. } => true,
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            scalar_expression_needs_no_bindings(left) && scalar_expression_needs_no_bindings(right)
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. }
        | CheckedScalarExpression::IntegerWrappingCast { operand, .. }
        | CheckedScalarExpression::RepresentationView { operand, .. }
        | CheckedScalarExpression::IntegerSaturatingCast { operand, .. } => {
            scalar_expression_needs_no_bindings(operand)
        }
        CheckedScalarExpression::Boolean(expression) => {
            boolean_expression_needs_no_bindings(expression)
        }
        CheckedScalarExpression::StructuralParameterByteLength { .. }
        | CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::ErasedParameter { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::StructuralParameterIndexedRead { .. }
        | CheckedScalarExpression::IntegerTrappingCast { .. } => false,
    }
}

fn boolean_expression_needs_no_bindings(
    expression: &checked_trees::CheckedBooleanExpression,
) -> bool {
    use checked_trees::CheckedBooleanExpression;
    match expression {
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::Local { .. } => true,
        CheckedBooleanExpression::Not(operand) => boolean_expression_needs_no_bindings(operand),
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            boolean_expression_needs_no_bindings(left)
                && boolean_expression_needs_no_bindings(right)
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            scalar_expression_needs_no_bindings(left) && scalar_expression_needs_no_bindings(right)
        }
        CheckedBooleanExpression::StorageRead { .. }
        | CheckedBooleanExpression::ErasedParameter { .. }
        | CheckedBooleanExpression::StructuralParameterField { .. }
        | CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
        | CheckedBooleanExpression::IntegerMathComparison { .. }
        | CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::ByteSequencePredicate { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
    }
}
