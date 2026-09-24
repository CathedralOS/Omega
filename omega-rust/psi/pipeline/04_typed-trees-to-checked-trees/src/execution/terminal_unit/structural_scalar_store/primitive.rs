//! Primitive leaves: whole primitive storage and primitive elements.
//!
//! A primitive leaf is written by `WriteOnlyPrimitiveStore` over one checked
//! path from an exclusive borrowed parameter (or a whole initialized mutable
//! primitive local). The path composes fields, literal elements, and the
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
use super::super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, DataMember, Multiplicity, PrimitiveType, StatementNode,
    SymbolHandle, TypeConstraintNode, TypeReferenceNode, TypedTrees,
};

/// Find only initialized primitive storage established before this occurrence.
pub(in crate::execution::terminal_unit) fn primitive_local_before<'program>(
    program: &'program TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<&'program typed_trees::statement::TableLocalData> {
    let mut locals = program
        .statement_table
        .statements(state.statement_nodes)
        .get(..statement_index)?
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
            _ => None,
        });
    let local = locals.next()?;
    if locals.next().is_some()
        || !local.is_mutable
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
        || !matches!(
            program
                .type_reference_table
                .type_reference(local.type_reference),
            TypeReferenceNode::Named { .. }
        )
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
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<CheckedUnitEffectOperationPlan> {
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        usize::try_from(statement_index).ok()?,
        assignment.target,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    // An erased borrow carrier names its captured referent's storage:
    // substitute the checked alias root and prepend its captured projection.
    // The scalar binding's destination keeps the authored root spelling.
    let authored_symbol = symbol;
    let (symbol, segments) =
        match super::super::receiver_aliases::aliases(program, facts, machine, state)
            .unwrap_or_default()
            .iter()
            .find(|alias| alias.owner == symbol)
        {
            Some(alias) => {
                let mut segments = alias.segments.clone();
                segments.extend_from_slice(&place.segments);
                (alias.root, segments)
            }
            None => (symbol, place.segments.clone()),
        };
    let path = primitive_path(
        program,
        facts,
        machine,
        state,
        statement_index,
        assignment.target,
        &segments,
    )?;
    let whole = path.is_empty();
    let (destination, primitive_type) = if let Some(local) = primitive_local_before(
        program,
        state,
        usize::try_from(statement_index).ok()?,
        symbol,
    ) {
        if !whole {
            return None;
        }
        (
            checked_trees::CheckedPrimitiveStoreDestination::Local { symbol },
            program.primitive_type_reference(local.type_reference)?,
        )
    } else {
        let (parameter_index, destination) =
            structural_parameters
                .iter()
                .enumerate()
                .find(|(_, destination)| {
                    program
                        .state_parameters(state)
                        .get(destination.position as usize)
                        .is_some_and(|parameter| parameter.symbol == symbol)
                })?;
        if (whole
            && (destination.is_self || destination.multiplicity != Multiplicity::Unrestricted))
            || destination.multiplicity == Multiplicity::Linear
            || !destination.qualifications.is_empty()
            || !matches!(
                destination.access,
                CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
            )
        {
            return None;
        }
        let parameter = program
            .state_parameters(state)
            .get(destination.position as usize)?;
        if parameter.is_self != destination.is_self || parameter.is_const || !parameter.is_mutable {
            return None;
        }
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return None;
        };
        let expected_access = match access {
            language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
            language_semantics::ReferenceAccess::WriteOnly => {
                CheckedStructuralAccess::WriteOnlyBorrow
            }
            language_semantics::ReferenceAccess::Shared => return None,
        };
        if destination.access != expected_access
            || (whole
                && !matches!(
                    program.type_reference_table.type_reference(*referee),
                    TypeReferenceNode::Named { .. }
                ))
        {
            return None;
        }
        (
            checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            program.primitive_type_reference(if whole {
                *referee
            } else {
                validation::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    assignment.target,
                )?
            })?,
        )
    };
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
        return Some(CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index,
            destination,
            path,
            value: checked_trees::CheckedCallScalarArgument::Computation(root.root),
        });
    }
    let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    if binding.expression != assignment.value
        || binding.destination
            != if whole {
                authored_symbol
            } else {
                SymbolHandle::invalid()
            }
        || crate::values::scalar_expression_type(value) != Some(primitive_type)
        || !scalar_store_value_is_exact(program, facts, state, binding, value, primitive_type)
    {
        return None;
    }
    Some(CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
        statement_index,
        destination,
        path,
        value: checked_trees::CheckedCallScalarArgument::Pure(value.clone()),
    })
}

/// The checked path from the destination root to its primitive leaf: empty
/// for whole primitive storage, otherwise fields, literal elements and
/// runtime elements ending at an array element. Every selector of the
/// target is admitted by `TargetSelectors`; a runtime one becomes the
/// statement's `RuntimeIndex(AssignmentIndex { depth })` segment.
fn primitive_path(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    target: typed_trees::expression::ExpressionHandle,
    segments: &[facts::PlaceSegment],
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    let Some(last) = segments.last() else {
        return Some(Vec::new());
    };
    // A field leaf is the scalar field store's; this store ends at an element.
    if !matches!(
        last,
        facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. }
    ) || !validation::place_has_builtin_coordinates(program, machine, Some(state), target)
    {
        return None;
    }
    primitive_leaf(program, machine, state, target)?;
    let selectors = super::selectors::TargetSelectors::resolve(
        program,
        facts,
        machine,
        state,
        statement_index,
        target,
    )?;
    checked_unit_path(program, &selectors, segments)
}

/// The assignment leaf's declared type must be a named primitive, after the
/// arithmetic-policy shell peel.
fn primitive_leaf(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<PrimitiveType> {
    let mut leaf = validation::declared_place_type_raw(program, machine, Some(state), expression)?;
    // An arithmetic-policy shell (`i32 in Wrapping`) qualifies the element's
    // operations, not its storage identity, so it peels here like the primitive
    // leaf of the record-field store route. Range, named, and domain
    // constraints carry their own write obligations and keep their own owners.
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(leaf)
    {
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
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(leaf)
    else {
        return None;
    };
    let atom = program.symbols.builtin_type_atom(*symbol)?;
    if name.as_str() != atom.symbol_name() {
        return None;
    }
    program.primitive_type_reference(leaf)
}

/// Literal path segments keep their exact source identity; fields reject an
/// erased or domain-constrained declaration; a runtime element names its
/// selector's retained coordinate.
fn checked_unit_path(
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
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
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
        | CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
    }
}
