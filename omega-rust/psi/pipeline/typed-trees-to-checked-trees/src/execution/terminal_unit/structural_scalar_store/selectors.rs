//! The indexed steps of one assignment target.
//!
//! Every indexed step of a target is an element selection: a literal selector
//! names a static element and must lie within its array's declared extent; a
//! runtime selector is the statement's retained `AssignmentIndex { depth }`
//! scalar (depth counted from the target inward, the coordinate
//! `validation::assignment_target_selectors` defines) and becomes a
//! `RuntimeIndex` path segment. The planner proves no bound for a runtime
//! element: Terminal re-proves `index < extent` for the segment's obligation.
use super::super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedUnitStructuralPathSegment, ExpressionNode,
    PrimitiveType, TypeReferenceNode, TypedTrees,
};
use typed_trees::expression::ExpressionHandle;

pub(super) struct TargetSelectors {
    /// The target's selector expressions from the target inward.
    selectors: Vec<ExpressionHandle>,
}

impl TargetSelectors {
    /// Admit every indexed step of `target`, or `None` when one of them is
    /// not an element of a declared fixed array: a range (a subslice), a
    /// literal outside its extent, a selector into a byte view or bounded
    /// byte field (those keep their live-length stores), or a runtime
    /// selector whose scalar coordinate the statement does not retain.
    pub(super) fn resolve(
        program: &TypedTrees,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: u32,
        target: ExpressionHandle,
    ) -> Option<Self> {
        let selectors = validation::assignment_target_selectors(program, target);
        let mut cursor = target;
        let mut depth = 0usize;
        loop {
            match program.expression_table.expression(cursor) {
                ExpressionNode::Indexed(indexed) => {
                    if selectors.get(depth) != Some(&indexed.index) {
                        return None;
                    }
                    let collection = validation::declared_place_type_raw(
                        program,
                        machine,
                        Some(state),
                        indexed.collection,
                    )?;
                    let collection = validation::unwrapped_type_reference(program, collection)?;
                    let TypeReferenceNode::FixedArray {
                        length: typed_trees::types::FixedArrayLength::Literal(length),
                        ..
                    } = program.type_reference_table.type_reference(collection)
                    else {
                        return None;
                    };
                    match program.expression_table.expression(indexed.index) {
                        ExpressionNode::Integer(literal) => {
                            if usize::try_from(literal.value_bignum()?.to_u64()?).ok()? >= *length {
                                return None;
                            }
                        }
                        _ => retained(
                            facts,
                            machine,
                            state,
                            statement_index,
                            u32::try_from(depth).ok()?,
                            indexed.index,
                        )?,
                    }
                    depth += 1;
                    cursor = indexed.collection;
                }
                ExpressionNode::Member(member) => cursor = member.receiver,
                ExpressionNode::Name(_) => break,
                _ => return None,
            }
        }
        (depth == selectors.len()).then_some(Self { selectors })
    }

    /// The checked segment of the runtime element `expression` selects.
    pub(super) fn runtime_segment(
        &self,
        expression: ExpressionHandle,
    ) -> Option<CheckedUnitStructuralPathSegment> {
        let depth = self
            .selectors
            .iter()
            .position(|selector| *selector == expression)?;
        Some(CheckedUnitStructuralPathSegment::RuntimeIndex(
            checked_trees::CheckedRuntimeIndex::AssignmentIndex {
                depth: u32::try_from(depth).ok()?,
            },
        ))
    }
}

/// The statement's `AssignmentIndex { depth }` coordinate retains exactly this
/// authored selector at an integer carrier: a bound pure expression, or the
/// computation root the ordinary scalar evaluator completes.
///
/// One admission limit is about proof cost, not meaning. A selector narrower
/// than Terminal's `u64` coordinate reaches the store through a widening or
/// exact cast, and re-proving `index < extent` through that conversion *and*
/// the selector's own arithmetic outgrows the c2l integer proof producers:
/// an unprovable goal such as `widen(y * 4 + x) < 12` searches for minutes
/// before failing (C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT). Until that search
/// is bounded, a narrower selector must be a parameter or a stored field
/// read, whose published or stored bound crosses the conversion in one step;
/// a local (including the temporary the front end hoists `a[y * 4 + x]`
/// into) carries its initializer's arithmetic into the goal. A `u64`
/// selector needs no conversion and composes freely.
fn retained(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    depth: u32,
    index: ExpressionHandle,
) -> Option<()> {
    let integer = |primitive_type: Option<PrimitiveType>| {
        matches!(
            primitive_type,
            Some(
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            )
        )
    };
    let role = CheckedScalarExpressionRole::AssignmentIndex { depth };
    if let Some((binding, value)) =
        facts
            .values
            .scalar_expressions
            .bound_expression_at(state.symbol, statement_index, role)
    {
        let primitive_type = crate::values::scalar_expression_type(value);
        return (binding.expression == index
            && integer(primitive_type)
            && (primitive_type == Some(PrimitiveType::U64) || direct_selector(value)))
        .then_some(());
    }
    let computations = &facts.values.scalar_computations;
    let root = computations.root_at(state.symbol, statement_index, role)?;
    if root.machine != machine.symbol || !computations.nodes.is_valid(root.root) {
        return None;
    }
    let node = computations.nodes.get(root.root);
    (node.authored_root == index
        && integer(Some(node.primitive_type))
        && (node.primitive_type == PrimitiveType::U64
            || match &node.kind {
                checked_trees::CheckedScalarComputationKind::StructuralField { .. } => true,
                checked_trees::CheckedScalarComputationKind::Value(value) => direct_selector(value),
                _ => false,
            }))
    .then_some(())
}

/// A selector value that is passed or read from a stored field, not
/// computed in this body.
fn direct_selector(value: &checked_trees::CheckedScalarExpression) -> bool {
    matches!(
        value,
        checked_trees::CheckedScalarExpression::Parameter { .. }
            | checked_trees::CheckedScalarExpression::StructuralParameterField { .. }
    )
}
