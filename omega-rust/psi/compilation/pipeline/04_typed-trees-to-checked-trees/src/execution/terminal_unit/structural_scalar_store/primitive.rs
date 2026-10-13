//! Primitive leaves: whole primitive storage and primitive elements.
//!
//! Restricted primitive stores retain their complete declaration and the exact
//! source assignment judgment. Bounded declarations additionally retain the
//! declaration-selected interval; general scalar predicates use the same
//! completed source-check cut without manufacturing a numeric receipt.
//! Terminal obligations keep the original restriction and assignment identity,
//! so carrier normalization cannot erase the receiving domain. Declared record
//! fields remain field operations with their own retained restrictions.
//!
//! A primitive leaf is the `Primitive` store leaf over one checked
//! path from the shared destination resolver's borrowed or owned storage.
//! The path composes fields, explicit reference crossings, literal elements, and the
//! target's runtime elements: `self.cells[self.i] = v` is the store over
//! `[cells, RuntimeIndex(AssignmentIndex { depth: 0 })]` and
//! `self.grid[i][j] = v` the store over `[grid, RuntimeIndex(depth 1),
//! RuntimeIndex(depth 0)]`, not a separate indexed store with a selector as
//! an operand. Each runtime selector retains the statement's exact
//! `AssignmentIndex { depth }` scalar and its separate bounds obligation.
//! The planner creates no bound from the selector's syntactic shape.
//!
//! Whole primitive array construction and replacement are sequenced by the
//! statement owner: its complete value is established before receiving storage
//! changes, so stored reads cannot observe a partial replacement.
use super::super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedUnitEffectOperationPlan, Multiplicity, PrimitiveType, StatementNode, SymbolHandle,
    TypeConstraintNode, TypeReferenceNode,
};

/// Find primitive storage established by an initializer or its declaration's
/// plain zero value before this occurrence.
/// Both binding kinds own contents; replacement permission is checked by
/// the store owner, while exact borrow facts decide the access a loan lends.
pub(in crate::execution::terminal_unit) fn primitive_local_before<'program>(
    program: &'program dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<&'program typed_trees::statement::TableLocalData> {
    let (_, local) = program
        .state_local_declaration(state, symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index)?;
    if (!program
        .expression_table
        .expression_is_valid(local.initial_value)
        && crate::values::zeroed_scalar_local_initializer(program, local).is_none())
        || !super::super::types::supports_primitive_storage(program, local.type_reference)
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
#[allow(clippy::too_many_arguments)]
pub(super) fn store_at(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &super::destination::Destination<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: super::value::AssignmentSource,
    trace: &crate::execution::terminal_unit::control::LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    let projection = destination.storage_projection(program)?;
    let primitive_type = program.primitive_type_reference(projection.type_reference)?;
    // Declared scalar fields retain their own field-store proof obligations.
    // Following a reference instead reaches primitive storage, regardless of
    // whether the pointer lives at the root or inside another owned value.
    if projection.ends_at_field || destination.recast_view.is_some() {
        return None;
    }
    let leaf = primitive_leaf_type_reference(program, machine, state, assignment.target)?;
    let receiving_range =
        match super::super::types::retain_scalar_field(program, leaf, &[], primitive_type, None)? {
            checked_trees::CheckedUnitStructuralFieldType::Scalar(_)
                if validation::unrestricted_builtin_primitive(program, leaf)
                    == Some(primitive_type) =>
            {
                None
            }
            checked_trees::CheckedUnitStructuralFieldType::BoundedInteger(integer) => {
                let decision = facts.proof.integer_assignment_at(
                    machine.symbol,
                    state.symbol,
                    statement_index as usize,
                    assignment.target,
                    assignment.value,
                );
                if !decision.is_valid() {
                    return None;
                }
                Some(checked_trees::CheckedPrimitiveReceivingRange {
                    primitive: primitive_type,
                    integer,
                    decision,
                })
            }
            // The declaration retains the complete predicate. Unlike an
            // interval it has no integer-assignment shortcut. The completed
            // source-check cut owns the full receiving predicate for this RHS.
            checked_trees::CheckedUnitStructuralFieldType::RefinedScalar { .. } => None,
            // Membership domains such as Finite deliberately remain plain scalar
            // fields under their invariant-window custody. That field judgment
            // cannot authorize a primitive store that discarded the membership.
            // Arbitrary predicates and atomic cells also retain their own owners.
            _ => return None,
        };
    if projection.path.is_empty() {
        match destination.root {
            super::destination::Root::Parameter { plan, .. }
                if plan.multiplicity != Multiplicity::Unrestricted =>
            {
                return None;
            }
            super::destination::Root::Local { symbol } => {
                // A local root names a `let mut` local or, when the
                // destination found its storage row, a `mut` scalar formal.
                if primitive_local_before(program, state, statement_index as usize, symbol)
                    .is_none_or(|local| !local.is_mutable)
                    && !program
                        .state_parameters(state)
                        .iter()
                        .any(|parameter| parameter.symbol == symbol && parameter.is_mutable)
                {
                    return None;
                }
            }
            _ => {}
        }
    }
    let authored = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index as usize,
        assignment.target,
    )?;
    let authored_symbol = authored.root.as_symbol()?;
    let value = match source {
        super::value::AssignmentSource::Authored => primitive_store_value(
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment,
            authored_symbol,
            authored.segments.is_empty(),
            primitive_type,
        )?,
        // An assignment that performs a scalar call owns its completed SSA
        // result instead of an AssignmentValue expression-table row. Retain
        // the same exact result-type and authored-call checks as field stores;
        // destination authority, selectors and receiving domains stay above.
        super::value::AssignmentSource::CallResult { .. } => super::value::assignment_value(
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment,
            source,
            primitive_type,
            trace,
        )?,
        super::value::AssignmentSource::StructuralCallResult { .. } => return None,
    };
    Some(CheckedUnitEffectOperationPlan::new_store(
        statement_index,
        destination.root.destination(),
        projection.path,
        checked_trees::CheckedStoreLeaf::Primitive { receiving_range },
        checked_trees::CheckedStoreValue::assigned(value),
    ))
}

/// The stored value of one primitive store: the statement's own call result
/// or the pure scalar bound at `AssignmentValue`, retagged to the leaf's
/// declared primitive. Whole-root stores keep the authored root symbol as
/// the bound destination; every element or field store binds `invalid`.
fn primitive_store_value(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    authored_symbol: SymbolHandle,
    authored_whole: bool,
    primitive_type: PrimitiveType,
) -> Option<checked_trees::CheckedScalarStoreValue> {
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
            || !computations.is_authored_root(
                root,
                machine.symbol,
                assignment.value,
                primitive_type,
            )
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
        return Some(checked_trees::CheckedScalarStoreValue::Computation(
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
    Some(checked_trees::CheckedScalarStoreValue::Pure(value.clone()))
}

/// The type of the primitive leaf an assignment through `expression` writes.
/// An arithmetic-policy shell (`i32 in Wrapping`) qualifies the element's
/// operations, not its storage identity, so it peels here like the primitive
/// leaf of the record-field store route. A represented integer interval keeps
/// its receiving judgment on the checked store leaf; other constraints decline.
/// Storage identity itself looks through qualification (the shared structural
/// identity contract); a successful source receiving judgment is required
/// before lowering that store. This does not create a Terminal field invariant.
/// An exclusive borrow carrier peels to its referee the same way: `borrowed`
/// in `let borrowed: &mut u8 = &mut self.buf[i]` spells the reference type,
/// while the store reaches the element behind it. Whether that reach is
/// lawful is the destination resolver's call, not the leaf walker's — a
/// carrier without validated formation keeps its own root and declines
/// there. A shared borrow never reaches this lane.
fn primitive_leaf_type_reference(
    program: &dyn ::validation::CheckingInput,
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

/// The scalar-binding closure both store custody proofs share: the binding
/// must close over exactly the primitive parameters and the immutable
/// primitive locals declared before its statement — nothing missing,
/// nothing extra.
fn scalar_binding_symbols_exact(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
        return matches!(program.statement_table.statement_at(state, statement_index),
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
    program: &dyn ::validation::CheckingInput,
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
        | CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::ImplementationSelection { .. } => true,
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            scalar_expression_needs_no_bindings(left) && scalar_expression_needs_no_bindings(right)
        }
        checked_trees::integer_widen_triple_pattern!(operand, ..)
        | CheckedScalarExpression::RepresentationView { operand, .. } => {
            scalar_expression_needs_no_bindings(operand)
        }
        CheckedScalarExpression::Boolean(expression) => {
            boolean_expression_needs_no_bindings(expression)
        }
        CheckedScalarExpression::StructuralSequenceLength { .. }
        | CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::ErasedParameter { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::StructuralParameterIndexedRead { .. } => false,
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
        facts::checked_boolean_triple_pattern!(left, right, ..) => {
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
